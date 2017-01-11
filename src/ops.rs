use md6;
use std::io;
use std::iter;
use time::strftime;
use iron::mime::Mime;
use std::sync::RwLock;
use lazysort::SortedBy;
use std::path::PathBuf;
use std::fs::{self, File};
use std::default::Default;
use iron::modifiers::Header;
use std::collections::HashMap;
use self::super::{Options, Error};
use mime_guess::guess_mime_type_opt;
use iron::{headers, status, method, mime, IronResult, Listening, Response, TypeMap, Request, Handler, Iron};
use self::super::util::{url_path, file_hash, is_symlink, encode_str, encode_file, hash_string, html_response, file_binary, percent_decode, response_encoding,
                        detect_file_as_dir, encoding_extension, file_time_modified, human_readable_size, USER_AGENT, ERROR_HTML, INDEX_EXTENSIONS,
                        MAX_ENCODING_SIZE, MIN_ENCODING_SIZE, DIRECTORY_LISTING_HTML};


// TODO: ideally this String here would be Encoding instead but hyper is bad
type CacheT<Cnt> = HashMap<([u8; 32], String), Cnt>;

pub struct HttpHandler {
    pub hosted_directory: (String, PathBuf),
    pub follow_symlinks: bool,
    pub check_indices: bool,
    pub writes_temp_dir: Option<(String, PathBuf)>,
    pub encoded_temp_dir: Option<(String, PathBuf)>,
    cache_gen: RwLock<CacheT<Vec<u8>>>,
    cache_fs: RwLock<CacheT<PathBuf>>,
}

impl HttpHandler {
    pub fn new(opts: &Options) -> HttpHandler {
        HttpHandler {
            hosted_directory: opts.hosted_directory.clone(),
            follow_symlinks: opts.follow_symlinks,
            check_indices: opts.check_indices,
            writes_temp_dir: HttpHandler::temp_subdir(&opts.temp_directory, opts.allow_writes, "writes"),
            encoded_temp_dir: HttpHandler::temp_subdir(&opts.temp_directory, opts.encode_fs, "encoded"),
            cache_gen: Default::default(),
            cache_fs: Default::default(),
        }
    }

    fn temp_subdir(td: &Option<(String, PathBuf)>, flag: bool, name: &str) -> Option<(String, PathBuf)> {
        if flag && td.is_some() {
            let &(ref temp_name, ref temp_dir) = td.as_ref().unwrap();
            Some((format!("{}{}{}",
                          temp_name,
                          if temp_name.ends_with("/") || temp_name.ends_with(r"\") {
                              ""
                          } else {
                              "/"
                          },
                          name),
                  temp_dir.join(name)))
        } else {
            None
        }
    }
}

impl Handler for HttpHandler {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        match req.method {
            method::Options => self.handle_options(req),
            method::Get => self.handle_get(req),
            method::Put => self.handle_put(req),
            method::Delete => self.handle_delete(req),
            method::Head => {
                self.handle_get(req).map(|mut r| {
                    r.body = None;
                    r
                })
            }
            method::Trace => self.handle_trace(req),
            _ => self.handle_bad_method(req),
        }
    }
}

impl HttpHandler {
    fn handle_options(&self, req: &mut Request) -> IronResult<Response> {
        println!("{} asked for options", req.remote_addr);
        Ok(Response::with((status::NoContent,
                           Header(headers::Server(USER_AGENT.to_string())),
                           Header(headers::Allow(vec![method::Options, method::Get, method::Put, method::Delete, method::Head, method::Trace])))))
    }

    fn handle_get(&self, req: &mut Request) -> IronResult<Response> {
        let (req_p, symlink, url_err) = self.parse_requested_path(req);

        if url_err {
            self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>")
        } else if !req_p.exists() || (symlink && !self.follow_symlinks) {
            self.handle_nonexistant(req, req_p)
        } else if req_p.is_file() {
            self.handle_get_file(req, req_p)
        } else {
            self.handle_get_dir(req, req_p)
        }
    }

    fn handle_invalid_url(&self, req: &mut Request, cause: &str) -> IronResult<Response> {
        println!("{} requested to {} {} with invalid URL -- {}",
                 req.remote_addr,
                 req.method,
                 req.url,
                 cause.replace("<p>", "").replace("</p>", ""));


        self.handle_generated_response_encoding(req,
                                                status::BadRequest,
                                                html_response(ERROR_HTML, &["400 Bad Request", "The request URL was invalid.", cause]))
    }

    fn handle_nonexistant(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        println!("{} requested to {} nonexistant entity {}", req.remote_addr, req.method, req_p.display());
        let url_p = url_path(&req.url);
        self.handle_generated_response_encoding(req,
                                                status::NotFound,
                                                html_response(ERROR_HTML,
                                                              &["404 Not Found", &format!("The requested entity \"{}\" doesn't exist.", url_p), ""]))
    }

    fn handle_get_file(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let mime_type = guess_mime_type_opt(&req_p).unwrap_or_else(|| if file_binary(&req_p) {
            "application/octet-stream".parse().unwrap()
        } else {
            "text/plain".parse().unwrap()
        });
        println!("{} was served file {} as {}", req.remote_addr, req_p.display(), mime_type);

        let flen = req_p.metadata().unwrap().len();
        if self.encoded_temp_dir.is_some() && flen > MIN_ENCODING_SIZE && flen < MAX_ENCODING_SIZE {
            self.handle_get_file_encoded(req, req_p, mime_type)
        } else {
            Ok(Response::with((status::Ok,
                               Header(headers::Server(USER_AGENT.to_string())),
                               Header(headers::LastModified(headers::HttpDate(file_time_modified(&req_p)))),
                               req_p,
                               mime_type)))
        }
    }

    fn handle_get_file_encoded(&self, req: &mut Request, req_p: PathBuf, mt: Mime) -> IronResult<Response> {
        if let Some(encoding) = req.headers.get_mut::<headers::AcceptEncoding>().and_then(|es| response_encoding(&mut **es)) {
            self.create_temp_dir(&self.encoded_temp_dir);
            let cache_key = (file_hash(&req_p), encoding.to_string());

            {
                if let Some(resp_p) = self.cache_fs.read().unwrap().get(&cache_key) {
                    println!("{} encoded as {} for {:.1}% ratio (cached)",
                             iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                             encoding,
                             ((req_p.metadata().unwrap().len() as f64) / (resp_p.metadata().unwrap().len() as f64)) * 100f64);

                    return Ok(Response::with((status::Ok,
                                              Header(headers::Server(USER_AGENT.to_string())),
                                              Header(headers::ContentEncoding(vec![encoding])),
                                              resp_p.as_path(),
                                              mt)));
                }
            }

            let mut resp_p = self.encoded_temp_dir.as_ref().unwrap().1.join(hash_string(&cache_key.0));
            match (req_p.extension(), encoding_extension(&encoding)) {
                (Some(ext), Some(enc)) => resp_p.set_extension(format!("{}.{}", ext.to_str().unwrap_or("ext"), enc)),
                (Some(ext), None) => resp_p.set_extension(format!("{}.{}", ext.to_str().unwrap_or("ext"), encoding)),
                (None, Some(enc)) => resp_p.set_extension(enc),
                (None, None) => resp_p.set_extension(format!("{}", encoding)),
            };

            if encode_file(&req_p, &resp_p, &encoding) {
                println!("{} encoded as {} for {:.1}% ratio",
                         iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                         encoding,
                         ((req_p.metadata().unwrap().len() as f64) / (resp_p.metadata().unwrap().len() as f64)) * 100f64);

                let mut cache = self.cache_fs.write().unwrap();
                cache.insert(cache_key, resp_p.clone());

                return Ok(Response::with((status::Ok,
                                          Header(headers::Server(USER_AGENT.to_string())),
                                          Header(headers::ContentEncoding(vec![encoding])),
                                          resp_p.as_path(),
                                          mt)));
            } else {
                println!("{} failed to encode as {}, sending identity",
                         iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                         encoding);
            }
        }

        Ok(Response::with((status::Ok,
                           Header(headers::Server(USER_AGENT.to_string())),
                           Header(headers::LastModified(headers::HttpDate(file_time_modified(&req_p)))),
                           req_p,
                           mt)))
    }

    fn handle_get_dir(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        if self.check_indices {
            let mut idx = req_p.join("index");
            if let Some(e) = INDEX_EXTENSIONS.iter()
                .find(|e| {
                    idx.set_extension(e);
                    idx.exists()
                }) {
                if req.url.path().pop() == Some("") {
                    let r = self.handle_get_file(req, idx);
                    println!("{} found index file for directory {}",
                             iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                             req_p.display());
                    return r;
                } else {
                    return self.handle_get_dir_index_no_slash(req, e);
                }
            }
        }

        self.handle_get_dir_listing(req, req_p)
    }

    fn handle_get_dir_index_no_slash(&self, req: &mut Request, idx_ext: &str) -> IronResult<Response> {
        let new_url = req.url.to_string() + "/";
        println!("Redirecting {} to {} - found index file index.{}", req.remote_addr, new_url, idx_ext);

        // We redirect here because if we don't and serve the index right away funky shit happens.
        // Example:
        //   - Without following slash:
        //     https://cloud.githubusercontent.com/assets/6709544/21442017/9eb20d64-c89b-11e6-8c7b-888b5f70a403.png
        //   - With following slash:
        //     https://cloud.githubusercontent.com/assets/6709544/21442028/a50918c4-c89b-11e6-8936-c29896947f6a.png
        Ok(Response::with((status::MovedPermanently, Header(headers::Server(USER_AGENT.to_string())), Header(headers::Location(new_url)))))
    }

    fn handle_get_dir_listing(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let relpath = (url_path(&req.url) + "/").replace("//", "/");
        let is_root = &req.url.path() == &[""];
        println!("{} was served directory listing for {}", req.remote_addr, req_p.display());
        self.handle_generated_response_encoding(req,
                                                status::Ok,
                                                html_response(DIRECTORY_LISTING_HTML,
                                                              &[&relpath,
                                                                &if self.writes_temp_dir.is_some() {
                                                                    r#"<script type="text/javascript">{drag_drop}</script>"#.to_string()
                                                                } else {
                                                                    String::new()
                                                                },
                                                                &if is_root {
                                                                    String::new()
                                                                } else {
                                                                    format!("<tr><td><a href=\"../\"><img id=\"parent_dir\" \
                                                                             src=\"{{back_arrow_icon}}\"></img></a></td> <td><a href=\"../\">Parent \
                                                                             directory</a></td> <td>{}</td> <td></td></tr>",
                                                                            strftime("%F %T", &file_time_modified(req_p.parent().unwrap())).unwrap())
                                                                },
                                                                &req_p.read_dir()
                                                                    .unwrap()
                                                                    .map(Result::unwrap)
                                                                    .filter(|f| self.follow_symlinks || !is_symlink(f.path()))
                                                                    .sorted_by(|lhs, rhs| {
                                                                        (lhs.file_type().unwrap().is_file(), lhs.file_name().to_str().unwrap().to_lowercase())
                                                                            .cmp(&(rhs.file_type().unwrap().is_file(),
                                                                                   rhs.file_name().to_str().unwrap().to_lowercase()))
                                                                    })
                                                                    .fold("".to_string(), |cur, f| {
                let url = format!("/{}", relpath).replace("//", "/");
                let is_file = f.file_type().unwrap().is_file();
                let path = f.path();
                let fname = f.file_name().into_string().unwrap();
                let len = f.metadata().unwrap().len();
                let mime = if is_file {
                    match guess_mime_type_opt(&path) {
                        Some(mime::Mime(mime::TopLevel::Image, ..)) |
                        Some(mime::Mime(mime::TopLevel::Video, ..)) => "_image",
                        Some(mime::Mime(mime::TopLevel::Text, ..)) => "_text",
                        Some(mime::Mime(mime::TopLevel::Application, ..)) => "_binary",
                        None => if file_binary(&path) { "" } else { "_text" },
                        _ => "",
                    }
                } else {
                    ""
                };

                format!("{}<tr><td><a href=\"{}{}\"><img id=\"{}\" src=\"{{{}{}_icon}}\"></img></a></td> <td><a href=\"{}{}\">{}{}</a></td> <td>{}</td> \
                         <td><abbr title=\"{} B\">{}</abbr></td></tr>\n",
                        cur,
                        url,
                        fname,
                        path.file_stem().map(|p| p.to_str().unwrap()).unwrap_or(&fname),
                        if is_file { "file" } else { "dir" },
                        mime,
                        url,
                        fname,
                        fname,
                        if is_file { "" } else { "/" },
                        strftime("%F %T", &file_time_modified(&path)).unwrap(),
                        if is_file {
                            len.to_string()
                        } else {
                            String::new()
                        },
                        if is_file {
                            human_readable_size(len)
                        } else {
                            String::new()
                        })
            })]))
    }

    fn handle_put(&self, req: &mut Request) -> IronResult<Response> {
        if self.writes_temp_dir.is_none() {
            return self.handle_forbidden_method(req, "-w", "write requests");
        }

        let (req_p, _, url_err) = self.parse_requested_path(req);

        if url_err {
            self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>")
        } else if req_p.is_dir() {
            self.handle_disallowed_method(req, &[method::Options, method::Get, method::Delete, method::Head, method::Trace], "directory")
        } else if detect_file_as_dir(&req_p) {
            self.handle_invalid_url(req, "<p>Attempted to use file as directory.</p>")
        } else if req.headers.has::<headers::ContentRange>() {
            self.handle_put_partial_content(req)
        } else {
            self.create_temp_dir(&self.writes_temp_dir);
            self.handle_put_file(req, req_p)
        }
    }

    fn handle_disallowed_method(&self, req: &mut Request, allowed: &[method::Method], tpe: &str) -> IronResult<Response> {
        let allowed_s = allowed.iter()
            .enumerate()
            .fold("".to_string(), |cur, (i, m)| {
                cur + &m.to_string() +
                if i == allowed.len() - 2 {
                    ", and "
                } else if i == allowed.len() - 1 {
                    ""
                } else {
                    ", "
                }
            })
            .to_string();

        println!("{} tried to {} on {} ({}) but only {} are allowed",
                 req.remote_addr,
                 req.method,
                 url_path(&req.url),
                 tpe,
                 allowed_s);

        let resp_text =
            html_response(ERROR_HTML,
                          &["405 Method Not Allowed", &format!("Can't {} on a {}.", req.method, tpe), &format!("<p>Allowed methods: {}</p>", allowed_s)]);
        self.handle_generated_response_encoding(req, status::MethodNotAllowed, resp_text)
            .map(|mut r| {
                r.headers.set(headers::Allow(allowed.to_vec()));
                r
            })
    }

    fn handle_put_partial_content(&self, req: &mut Request) -> IronResult<Response> {
        println!("{} tried to PUT partial content to {}", req.remote_addr, url_path(&req.url));
        self.handle_generated_response_encoding(req,
                                                status::BadRequest,
                                                html_response(ERROR_HTML,
                                                              &["400 Bad Request",
                                                                "<a href=\"https://tools.ietf.org/html/rfc7231#section-4.3.3\">RFC7231 forbids \
                                                                 partial-content PUT requests.</a>",
                                                                ""]))
    }

    fn handle_put_file(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let existant = req_p.exists();
        println!("{} {} {}, size: {}B",
                 req.remote_addr,
                 if existant { "replaced" } else { "created" },
                 req_p.display(),
                 *req.headers.get::<headers::ContentLength>().unwrap());

        let &(_, ref temp_dir) = self.writes_temp_dir.as_ref().unwrap();
        let temp_file_p = temp_dir.join(req_p.file_name().unwrap());

        io::copy(&mut req.body, &mut File::create(&temp_file_p).unwrap()).unwrap();
        let _ = fs::create_dir_all(req_p.parent().unwrap());
        fs::copy(&temp_file_p, req_p).unwrap();

        Ok(Response::with((if existant {
                               status::NoContent
                           } else {
                               status::Created
                           },
                           Header(headers::Server(USER_AGENT.to_string())))))
    }

    fn handle_delete(&self, req: &mut Request) -> IronResult<Response> {
        if self.writes_temp_dir.is_none() {
            return self.handle_forbidden_method(req, "-w", "write requests");
        }

        let (req_p, symlink, url_err) = self.parse_requested_path(req);

        if url_err {
            self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>")
        } else if !req_p.exists() || (symlink && !self.follow_symlinks) {
            self.handle_nonexistant(req, req_p)
        } else {
            self.handle_delete_path(req, req_p)
        }
    }

    fn handle_delete_path(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        println!("{} deleted {} {}",
                 req.remote_addr,
                 if req_p.is_file() { "file" } else { "directory" },
                 req_p.display());

        if req_p.is_file() {
            fs::remove_file(req_p).unwrap();
        } else {
            fs::remove_dir_all(req_p).unwrap();
        }

        Ok(Response::with((status::NoContent, Header(headers::Server(USER_AGENT.to_string())))))
    }

    fn handle_trace(&self, req: &mut Request) -> IronResult<Response> {
        println!("{} requested TRACE", req.remote_addr);

        let mut hdr = req.headers.clone();
        hdr.set(headers::ContentType("message/http".parse().unwrap()));

        Ok(Response {
            status: Some(status::Ok),
            headers: hdr,
            extensions: TypeMap::new(),
            body: None,
        })
    }

    fn handle_forbidden_method(&self, req: &mut Request, switch: &str, desc: &str) -> IronResult<Response> {
        println!("{} used disabled request method {} grouped under {}", req.remote_addr, req.method, desc);
        self.handle_generated_response_encoding(req,
                                                status::Forbidden,
                                                html_response(ERROR_HTML,
                                                              &["403 Forbidden",
                                                                "This feature is currently disabled.",
                                                                &format!("<p>Ask the server administrator to pass <samp>{}</samp> to the executable to \
                                                                          enable support for {}.</p>",
                                                                         switch,
                                                                         desc)]))
    }

    fn handle_bad_method(&self, req: &mut Request) -> IronResult<Response> {
        println!("{} used invalid request method {}", req.remote_addr, req.method);
        let last_p = format!("<p>Unsupported request method: {}.<br />\nSupported methods: OPTIONS, GET, PUT, DELETE, HEAD and TRACE.</p>",
                             req.method);
        self.handle_generated_response_encoding(req,
                                                status::NotImplemented,
                                                html_response(ERROR_HTML, &["501 Not Implemented", "This operation was not implemented.", &last_p]))
    }

    fn handle_generated_response_encoding(&self, req: &mut Request, st: status::Status, resp: String) -> IronResult<Response> {
        if let Some(encoding) = req.headers.get_mut::<headers::AcceptEncoding>().and_then(|es| response_encoding(&mut **es)) {
            let mut cache_key = ([0u8; 32], encoding.to_string());
            md6::hash(256, resp.as_bytes(), &mut cache_key.0).unwrap();

            {
                if let Some(enc_resp) = self.cache_gen.read().unwrap().get(&cache_key) {
                    println!("{} encoded as {} for {:.1}% ratio (cached)",
                             iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                             encoding,
                             ((resp.len() as f64) / (enc_resp.len() as f64)) * 100f64);

                    return Ok(Response::with((st,
                                              Header(headers::Server(USER_AGENT.to_string())),
                                              Header(headers::ContentEncoding(vec![encoding])),
                                              "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                                              &enc_resp[..])));
                }
            }

            if let Some(enc_resp) = encode_str(&resp, &encoding) {
                println!("{} encoded as {} for {:.1}% ratio",
                         iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                         encoding,
                         ((resp.len() as f64) / (enc_resp.len() as f64)) * 100f64);

                let mut cache = self.cache_gen.write().unwrap();
                cache.insert(cache_key.clone(), enc_resp);

                return Ok(Response::with((st,
                                          Header(headers::Server(USER_AGENT.to_string())),
                                          Header(headers::ContentEncoding(vec![encoding])),
                                          "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                                          &cache[&cache_key][..])));
            } else {
                println!("{} failed to encode as {}, sending identity",
                         iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                         encoding);
            }
        }

        Ok(Response::with((st, Header(headers::Server(USER_AGENT.to_string())), "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(), resp)))
    }

    fn parse_requested_path(&self, req: &Request) -> (PathBuf, bool, bool) {
        req.url.path().into_iter().filter(|p| !p.is_empty()).fold((self.hosted_directory.1.clone(), false, false), |(mut cur, mut sk, mut err), pp| {
            if let Some(pp) = percent_decode(pp) {
                cur.push(&*pp);
            } else {
                err = true;
            }

            while let Ok(newlink) = cur.read_link() {
                cur = newlink;
                sk = true;
            }

            (cur, sk, err)
        })
    }

    fn create_temp_dir(&self, td: &Option<(String, PathBuf)>) {
        let &(ref temp_name, ref temp_dir) = td.as_ref().unwrap();
        if !temp_dir.exists() && fs::create_dir_all(&temp_dir).is_ok() {
            println!("Created temp dir {}", temp_name);
        }
    }
}

impl Clone for HttpHandler {
    fn clone(&self) -> HttpHandler {
        HttpHandler {
            hosted_directory: self.hosted_directory.clone(),
            follow_symlinks: self.follow_symlinks,
            check_indices: self.check_indices,
            writes_temp_dir: self.writes_temp_dir.clone(),
            encoded_temp_dir: self.encoded_temp_dir.clone(),
            cache_gen: Default::default(),
            cache_fs: Default::default(),
        }
    }
}


/// Attempt to start a server on ports from `from` to `up_to`, inclusive, with the specified handler.
///
/// If an error other than the port being full is encountered it is returned.
///
/// If all ports from the range are not free an error is returned.
///
/// # Examples
///
/// ```
/// # extern crate https;
/// # extern crate iron;
/// # use https::ops::try_ports;
/// # use iron::{status, Response};
/// let server = try_ports(|req| Ok(Response::with((status::Ok, "Abolish the burgeoisie!"))), 8000, 8100).unwrap();
/// ```
pub fn try_ports<H: Handler + Clone>(hndlr: H, from: u16, up_to: u16) -> Result<Listening, Error> {
    for port in from..up_to + 1 {
        match Iron::new(hndlr.clone()).http(("0.0.0.0", port)) {
            Ok(server) => return Ok(server),
            Err(error) => {
                if !error.to_string().contains("port") {
                    return Err(Error::Io {
                        desc: "server",
                        op: "start",
                        more: None,
                    });
                }
            }
        }
    }

    Err(Error::Io {
        desc: "server",
        op: "start",
        more: Some("no free ports"),
    })
}
