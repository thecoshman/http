use std::io;
use lazysort::SortedBy;
use std::path::PathBuf;
use std::fs::{self, File};
use iron::modifiers::Header;
use self::super::{Options, Error};
use mime_guess::guess_mime_type_opt;
use iron::{headers, status, method, mime, IronResult, Listening, Response, TypeMap, Request, Handler, Iron};
use self::super::util::{url_path, is_symlink, html_response, file_contains, percent_decode, detect_file_as_dir, file_time_modified, USER_AGENT, ERROR_HTML,
                        DIRECTORY_LISTING_HTML};


#[derive(Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct HttpHandler {
    pub hosted_directory: (String, PathBuf),
    pub follow_symlinks: bool,
    pub temp_directory: Option<(String, PathBuf)>,
}

impl HttpHandler {
    pub fn new(opts: &Options) -> HttpHandler {
        HttpHandler {
            hosted_directory: opts.hosted_directory.clone(),
            follow_symlinks: opts.follow_symlinks,
            temp_directory: opts.temp_directory.clone(),
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

        Ok(Response::with((status::BadRequest,
                           Header(headers::Server(USER_AGENT.to_string())),
                           "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                           html_response(ERROR_HTML, &["400 Bad Request", "The request URL was invalid.", cause]))))
    }

    fn handle_nonexistant(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        println!("{} requested to {} nonexistant entity {}", req.remote_addr, req.method, req_p.display());
        Ok(Response::with((status::NotFound,
                           Header(headers::Server(USER_AGENT.to_string())),
                           "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                           html_response(ERROR_HTML,
                                         &["404 Not Found", &format!("The requested entity \"{}\" doesn't exist.", url_path(&req.url)), ""]))))
    }

    fn handle_get_file(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let mime_type = guess_mime_type_opt(&req_p).unwrap_or_else(|| if file_contains(&req_p, 0) {
            "application/octet-stream".parse().unwrap()
        } else {
            "text/plain".parse().unwrap()
        });
        println!("{} was served file {} as {}", req.remote_addr, req_p.display(), mime_type);
        Ok(Response::with((status::Ok,
                           Header(headers::Server(USER_AGENT.to_string())),
                           Header(headers::LastModified(headers::HttpDate(file_time_modified(&req_p)))),
                           mime_type,
                           req_p)))
    }

    fn handle_get_dir(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let relpath = (url_path(&req.url) + "/").replace("//", "/");
        println!("{} was served directory listing for {}", req.remote_addr, req_p.display());
        Ok(Response::with((status::Ok,
                           Header(headers::Server(USER_AGENT.to_string())),
                           "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                           html_response(DIRECTORY_LISTING_HTML,
                                         &[&relpath,
                                           &req_p.read_dir()
                                               .unwrap()
                                               .map(Result::unwrap)
                                               .filter(|f| self.follow_symlinks || !is_symlink(f.path()))
                                               .sorted_by(|lhs, rhs| {
                                                   (lhs.file_type().unwrap().is_file(), lhs.file_name().to_str().unwrap().to_lowercase())
                                                       .cmp(&(rhs.file_type().unwrap().is_file(), rhs.file_name().to_str().unwrap().to_lowercase()))
                                               })
                                               .fold("".to_string(), |cur, f| {
                let fname = f.file_name().into_string().unwrap() +
                            if !f.file_type().unwrap().is_file() {
                    "/"
                } else {
                    ""
                };
                cur + "<li><a href=\"" + &format!("/{}", relpath).replace("//", "/") + &fname + "\">" + &fname + "</a></li>\n"
            })]))))
    }

    fn handle_put(&self, req: &mut Request) -> IronResult<Response> {
        if self.temp_directory.is_none() {
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
            self.create_temp_dir();
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

        Ok(Response::with((status::MethodNotAllowed,
                           Header(headers::Server(USER_AGENT.to_string())),
                           Header(headers::Allow(allowed.to_vec())),
                           "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                           html_response(ERROR_HTML,
                                         &["405 Method Not Allowed",
                                           &format!("Can't {} on a {}.", req.method, tpe),
                                           &format!("<p>Allowed methods: {}</p>", allowed_s)]))))
    }

    fn handle_put_partial_content(&self, req: &mut Request) -> IronResult<Response> {
        println!("{} tried to PUT partial content to {}", req.remote_addr, url_path(&req.url));
        Ok(Response::with((status::BadRequest,
                           Header(headers::Server(USER_AGENT.to_string())),
                           "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                           html_response(ERROR_HTML,
                                         &["400 Bad Request",
                                           "<a href=\"https://tools.ietf.org/html/rfc7231#section-4.3.3\">RFC7231 forbids partial-content PUT \
                                            requests.</a>",
                                           ""]))))
    }

    fn handle_put_file(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let existant = req_p.exists();
        println!("{} {} {}, size: {}B",
                 req.remote_addr,
                 if existant { "replaced" } else { "created" },
                 req_p.display(),
                 *req.headers.get::<headers::ContentLength>().unwrap());

        let &(_, ref temp_dir) = self.temp_directory.as_ref().unwrap();
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
        if self.temp_directory.is_none() {
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
        Ok(Response::with((status::Forbidden,
                           Header(headers::Server(USER_AGENT.to_string())),
                           "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                           html_response(ERROR_HTML,
                                         &["403 Forbidden",
                                           "This feature is currently disabled.",
                                           &format!("<p>Ask the server administrator to pass <samp>{}</samp> \
                                                        to the executable to enable support for {}.</p>",
                                                    switch,
                                                    desc)]))))
    }

    fn handle_bad_method(&self, req: &mut Request) -> IronResult<Response> {
        println!("{} used invalid request method {}", req.remote_addr, req.method);
        Ok(Response::with((status::NotImplemented,
                           Header(headers::Server(USER_AGENT.to_string())),
                           "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                           html_response(ERROR_HTML,
                                         &["501 Not Implemented",
                                           "This operation was not implemented.",
                                           &format!("<p>Unsupported request method: {}.<br />\n\
                                                     Supported methods: OPTIONS, GET, PUT, DELETE, HEAD and TRACE.</p>",
                                                    req.method)]))))
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

    fn create_temp_dir(&self) {
        let &(ref temp_name, ref temp_dir) = self.temp_directory.as_ref().unwrap();
        if !temp_dir.exists() && fs::create_dir_all(&temp_dir).is_ok() {
            println!("Created temp dir {}", temp_name);
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
