use md6;
use std::iter;
use time::now;
use serde_json;
use std::borrow::Cow;
use serde::Serialize;
use unicase::UniCase;
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
use hyper_native_tls::NativeTlsServer;
use std::io::{self, SeekFrom, Write, Read, Seek};
use trivial_colours::{Reset as CReset, Colour as C};
use std::process::{ExitStatus, Command, Child, Stdio};
use rfsapi::{RawFsApiHeader, FilesetData, RawFileData};
use iron::{headers, status, method, mime, IronResult, Listening, Response, TypeMap, Request, Handler, Iron};
use self::super::util::{url_path, file_hash, is_symlink, encode_str, encode_file, file_length, hash_string, html_response, file_binary, client_mobile,
                        percent_decode, file_icon_suffix, is_actually_file, is_descendant_of, response_encoding, detect_file_as_dir, encoding_extension,
                        file_time_modified, get_raw_fs_metadata, human_readable_size, is_nonexistant_descendant_of, USER_AGENT, ERROR_HTML, INDEX_EXTENSIONS,
                        MIN_ENCODING_GAIN, MAX_ENCODING_SIZE, MIN_ENCODING_SIZE, DIRECTORY_LISTING_HTML, MOBILE_DIRECTORY_LISTING_HTML,
                        BLACKLISTED_ENCODING_EXTENSIONS};


macro_rules! log {
    ($fmt:expr) => {
        print!("{}[{}]{} ", C::Cyan, now().strftime("%F %T").unwrap(), CReset);
        println!(concat!($fmt, "{black:.0}{red:.0}{green:.0}{yellow:.0}{blue:.0}{magenta:.0}{cyan:.0}{white:.0}{reset:.0}"),
                 black = C::Black,
                 red = C::Red,
                 green = C::Green,
                 yellow = C::Yellow,
                 blue = C::Blue,
                 magenta = C::Magenta,
                 cyan = C::Cyan,
                 white = C::White,
                 reset = CReset);
    };
    ($fmt:expr, $($arg:tt)*) => {
        print!("{}[{}]{} ", C::Cyan, now().strftime("%F %T").unwrap(), CReset);
        println!(concat!($fmt, "{black:.0}{red:.0}{green:.0}{yellow:.0}{blue:.0}{magenta:.0}{cyan:.0}{white:.0}{reset:.0}"),
                 $($arg)*,
                 black = C::Black,
                 red = C::Red,
                 green = C::Green,
                 yellow = C::Yellow,
                 blue = C::Blue,
                 magenta = C::Magenta,
                 cyan = C::Cyan,
                 white = C::White,
                 reset = CReset);
    };
}


// TODO: ideally this String here would be Encoding instead but hyper is bad
type CacheT<Cnt> = HashMap<([u8; 32], String), Cnt>;

pub struct HttpHandler {
    pub hosted_directory: (String, PathBuf),
    pub follow_symlinks: bool,
    pub sandbox_symlinks: bool,
    pub check_indices: bool,
    pub writes_temp_dir: Option<(String, PathBuf)>,
    pub encoded_temp_dir: Option<(String, PathBuf)>,
    cache_gen: RwLock<CacheT<Vec<u8>>>,
    cache_fs: RwLock<CacheT<(PathBuf, bool)>>,
}

impl HttpHandler {
    pub fn new(opts: &Options) -> HttpHandler {
        HttpHandler {
            hosted_directory: opts.hosted_directory.clone(),
            follow_symlinks: opts.follow_symlinks,
            sandbox_symlinks: opts.sandbox_symlinks,
            check_indices: opts.check_indices,
            writes_temp_dir: HttpHandler::temp_subdir(&opts.temp_directory, opts.allow_writes, "writes"),
            encoded_temp_dir: HttpHandler::temp_subdir(&opts.temp_directory, opts.encode_fs, "encoded"),
            cache_gen: Default::default(),
            cache_fs: Default::default(),
        }
    }

    pub fn clean_temp_dirs(temp_dir: &(String, PathBuf)) {
        for (temp_name, temp_dir) in ["writes", "encoded", "tls"].into_iter().flat_map(|tn| HttpHandler::temp_subdir(temp_dir, true, tn)) {
            if temp_dir.exists() && fs::remove_dir_all(&temp_dir).is_ok() {
                log!("Deleted temp dir {magenta}{}{reset}", temp_name);
            }
        }
    }

    fn temp_subdir(&(ref temp_name, ref temp_dir): &(String, PathBuf), flag: bool, name: &str) -> Option<(String, PathBuf)> {
        if flag {
            Some((format!("{}{}{}",
                          temp_name,
                          if temp_name.ends_with('/') || temp_name.ends_with('\\') {
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
        log!("{green}{}{reset} asked for {red}OPTIONS{reset}", req.remote_addr);
        Ok(Response::with((status::NoContent,
                           Header(headers::Server(USER_AGENT.to_string())),
                           Header(headers::Allow(vec![method::Options, method::Get, method::Put, method::Delete, method::Head, method::Trace])))))
    }

    fn handle_get(&self, req: &mut Request) -> IronResult<Response> {
        let (req_p, symlink, url_err) = self.parse_requested_path(req);
        let file = is_actually_file(&req_p.metadata().expect("Failed to get file metadata").file_type());
        let range = req.headers.get().map(|r: &headers::Range| (*r).clone());
        let raw_fs = req.headers.get().map(|r: &RawFsApiHeader| r.0).unwrap_or(false);

        if url_err {
            self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>")
        } else if !req_p.exists() || (symlink && !self.follow_symlinks) ||
                  (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&req_p, &self.hosted_directory.1)) {
            self.handle_nonexistant(req, req_p)
        } else if file && raw_fs {
            self.handle_get_raw_fs_file(req, req_p)
        } else if file && range.is_some() {
            self.handle_get_file_range(req, req_p, range.unwrap())
        } else if file {
            self.handle_get_file(req, req_p)
        } else if raw_fs {
            self.handle_get_raw_fs_dir(req, req_p)
        } else {
            self.handle_get_dir(req, req_p)
        }
    }

    fn handle_invalid_url(&self, req: &mut Request, cause: &str) -> IronResult<Response> {
        log!("{green}{}{reset} requested to {red}{}{reset} {yellow}{}{reset} with invalid URL -- {}",
             req.remote_addr,
             req.method,
             req.url,
             cause.replace("<p>", "").replace("</p>", ""));

        self.handle_generated_response_encoding(req,
                                                status::BadRequest,
                                                html_response(ERROR_HTML, &["400 Bad Request", "The request URL was invalid.", cause]))
    }

    fn handle_nonexistant(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        log!("{green}{}{reset} requested to {red}{}{reset} nonexistant entity {magenta}{}{reset}",
             req.remote_addr,
             req.method,
             req_p.display());

        let url_p = url_path(&req.url);
        self.handle_generated_response_encoding(req,
                                                status::NotFound,
                                                html_response(ERROR_HTML,
                                                              &["404 Not Found", &format!("The requested entity \"{}\" doesn't exist.", url_p), ""]))
    }

    fn handle_get_raw_fs_file(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        log!("{green}{}{reset} was served metadata for file {magenta}{}{reset}",
             req.remote_addr,
             req_p.display());
        self.handle_raw_fs_api_response(status::Ok,
                                        &FilesetData {
                                            writes_supported: self.writes_temp_dir.is_some(),
                                            is_root: false,
                                            is_file: true,
                                            files: vec![get_raw_fs_metadata(&req_p)],
                                        })
    }

    fn handle_get_file_range(&self, req: &mut Request, req_p: PathBuf, range: headers::Range) -> IronResult<Response> {
        match range {
            headers::Range::Bytes(ref brs) => {
                if brs.len() == 1 {
                    let flen = file_length(&req_p.metadata().expect("Failed to get requested file metadata"), &req_p);
                    match brs[0] {
                        // Cases where from is bigger than to are filtered out by iron so can never happen
                        headers::ByteRangeSpec::FromTo(from, to) => self.handle_get_file_closed_range(req, req_p, from, to),
                        headers::ByteRangeSpec::AllFrom(from) => {
                            if flen < from {
                                self.handle_get_file_empty_range(req, req_p, from, flen)
                            } else {
                                self.handle_get_file_right_opened_range(req, req_p, from)
                            }
                        }
                        headers::ByteRangeSpec::Last(from) => {
                            if flen < from {
                                self.handle_get_file_empty_range(req, req_p, from, flen)
                            } else {
                                self.handle_get_file_left_opened_range(req, req_p, from)
                            }
                        }
                    }
                } else {
                    self.handle_invalid_range(req, req_p, &range, "More than one range is unsupported.")
                }
            }
            headers::Range::Unregistered(..) => self.handle_invalid_range(req, req_p, &range, "Custom ranges are unsupported."),
        }
    }

    fn handle_get_file_closed_range(&self, req: &mut Request, req_p: PathBuf, from: u64, to: u64) -> IronResult<Response> {
        let mime_type = guess_mime_type_opt(&req_p).unwrap_or_else(|| if file_binary(&req_p) {
            "application/octet-stream".parse().unwrap()
        } else {
            "text/plain".parse().unwrap()
        });
        log!("{green}{}{reset} was served byte range {}-{} of file {magenta}{}{reset} as {blue}{}{reset}",
             req.remote_addr,
             from,
             to,
             req_p.display(),
             mime_type);

        let mut buf = vec![0; (to + 1 - from) as usize];
        let mut f = File::open(&req_p).expect("Failed to open requested file");
        f.seek(SeekFrom::Start(from)).expect("Failed to seek requested file");
        f.read_exact(&mut buf).expect("Failed to read requested file");

        Ok(Response::with((status::PartialContent,
                           (Header(headers::Server(USER_AGENT.to_string())),
                            Header(headers::LastModified(headers::HttpDate(file_time_modified(&req_p)))),
                            Header(headers::ContentRange(headers::ContentRangeSpec::Bytes {
                                range: Some((from, to)),
                                instance_length: Some(file_length(&f.metadata().expect("Failed to get requested file metadata"), &req_p)),
                            })),
                            Header(headers::AcceptRanges(vec![headers::RangeUnit::Bytes]))),
                           buf,
                           mime_type)))
    }

    fn handle_get_file_right_opened_range(&self, req: &mut Request, req_p: PathBuf, from: u64) -> IronResult<Response> {
        let mime_type = guess_mime_type_opt(&req_p).unwrap_or_else(|| if file_binary(&req_p) {
            "application/octet-stream".parse().unwrap()
        } else {
            "text/plain".parse().unwrap()
        });
        log!("{green}{}{reset} was served file {magenta}{}{reset} from byte {} as {blue}{}{reset}",
             req.remote_addr,
             req_p.display(),
             from,
             mime_type);

        let flen = file_length(&req_p.metadata().expect("Failed to get requested file metadata"), &req_p);
        self.handle_get_file_opened_range(req_p, SeekFrom::Start(from), from, flen - from, mime_type)
    }

    fn handle_get_file_left_opened_range(&self, req: &mut Request, req_p: PathBuf, from: u64) -> IronResult<Response> {
        let mime_type = guess_mime_type_opt(&req_p).unwrap_or_else(|| if file_binary(&req_p) {
            "application/octet-stream".parse().unwrap()
        } else {
            "text/plain".parse().unwrap()
        });
        log!("{green}{}{reset} was served last {} bytes of file {magenta}{}{reset} as {blue}{}{reset}",
             req.remote_addr,
             from,
             req_p.display(),
             mime_type);

        let flen = file_length(&req_p.metadata().expect("Failed to get requested file metadata"), &req_p);
        self.handle_get_file_opened_range(req_p, SeekFrom::End(-(from as i64)), flen - from, from, mime_type)
    }

    fn handle_get_file_opened_range(&self, req_p: PathBuf, s: SeekFrom, b_from: u64, clen: u64, mt: Mime) -> IronResult<Response> {
        let mut f = File::open(&req_p).expect("Failed to open requested file");
        let flen = file_length(&f.metadata().expect("Failed to get requested file metadata"), &req_p);
        f.seek(s).expect("Failed to seek requested file");

        Ok(Response::with((status::PartialContent,
                           f,
                           (Header(headers::Server(USER_AGENT.to_string())),
                            Header(headers::LastModified(headers::HttpDate(file_time_modified(&req_p)))),
                            Header(headers::ContentRange(headers::ContentRangeSpec::Bytes {
                                range: Some((b_from, flen - 1)),
                                instance_length: Some(flen),
                            })),
                            Header(headers::ContentLength(clen)),
                            Header(headers::AcceptRanges(vec![headers::RangeUnit::Bytes]))),
                           mt)))
    }

    fn handle_invalid_range(&self, req: &mut Request, req_p: PathBuf, range: &headers::Range, reason: &str) -> IronResult<Response> {
        self.handle_generated_response_encoding(req,
                                                status::RangeNotSatisfiable,
                                                html_response(ERROR_HTML,
                                                              &["416 Range Not Satisfiable",
                                                                &format!("Requested range <samp>{}</samp> could not be fullfilled for file {}.",
                                                                         range,
                                                                         req_p.display()),
                                                                reason]))
    }

    fn handle_get_file_empty_range(&self, req: &mut Request, req_p: PathBuf, from: u64, to: u64) -> IronResult<Response> {
        let mime_type = guess_mime_type_opt(&req_p).unwrap_or_else(|| if file_binary(&req_p) {
            "application/octet-stream".parse().unwrap()
        } else {
            "text/plain".parse().unwrap()
        });
        log!("{green}{}{reset} was served an empty range from file {magenta}{}{reset} as {blue}{}{reset}",
             req.remote_addr,
             req_p.display(),
             mime_type);

        Ok(Response::with((status::NoContent,
                           Header(headers::Server(USER_AGENT.to_string())),
                           Header(headers::LastModified(headers::HttpDate(file_time_modified(&req_p)))),
                           Header(headers::ContentRange(headers::ContentRangeSpec::Bytes {
                               range: Some((from, to)),
                               instance_length: Some(file_length(&req_p.metadata().expect("Failed to get requested file metadata"), &req_p)),
                           })),
                           Header(headers::AcceptRanges(vec![headers::RangeUnit::Bytes])),
                           mime_type)))
    }

    fn handle_get_file(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let mime_type = guess_mime_type_opt(&req_p).unwrap_or_else(|| if file_binary(&req_p) {
            "application/octet-stream".parse().unwrap()
        } else {
            "text/plain".parse().unwrap()
        });
        log!("{green}{}{reset} was served file {magenta}{}{reset} as {blue}{}{reset}",
             req.remote_addr,
             req_p.display(),
             mime_type);

        let metadata = req_p.metadata().expect("Failed to get requested file metadata");
        let flen = file_length(&metadata, &req_p);
        if self.encoded_temp_dir.is_some() && flen > MIN_ENCODING_SIZE && flen < MAX_ENCODING_SIZE &&
           req_p.extension().and_then(|s| s.to_str()).map(|s| !BLACKLISTED_ENCODING_EXTENSIONS.contains(&UniCase::new(s))).unwrap_or(true) {
            self.handle_get_file_encoded(req, req_p, mime_type)
        } else {
            Ok(Response::with((status::Ok,
                               (Header(headers::Server(USER_AGENT.to_string())),
                                Header(headers::LastModified(headers::HttpDate(file_time_modified(&req_p)))),
                                Header(headers::AcceptRanges(vec![headers::RangeUnit::Bytes]))),
                               req_p.as_path(),
                               Header(headers::ContentLength(file_length(&metadata, &req_p))),
                               mime_type)))
        }
    }

    fn handle_get_file_encoded(&self, req: &mut Request, req_p: PathBuf, mt: Mime) -> IronResult<Response> {
        if let Some(encoding) = req.headers.get_mut::<headers::AcceptEncoding>().and_then(|es| response_encoding(&mut **es)) {
            self.create_temp_dir(&self.encoded_temp_dir);
            let cache_key = (file_hash(&req_p), encoding.to_string());

            {
                match self.cache_fs.read().expect("Filesystem cache read lock poisoned").get(&cache_key) {
                    Some(&(ref resp_p, true)) => {
                        log!("{} encoded as {} for {:.1}% ratio (cached)",
                             iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                             encoding,
                             ((file_length(&req_p.metadata().expect("Failed to get requested file metadata"), &req_p) as f64) /
                              (file_length(&resp_p.metadata().expect("Failed to get encoded file metadata"), &resp_p) as f64)) *
                             100f64);

                        return Ok(Response::with((status::Ok,
                                                  Header(headers::Server(USER_AGENT.to_string())),
                                                  Header(headers::ContentEncoding(vec![encoding])),
                                                  Header(headers::AcceptRanges(vec![headers::RangeUnit::Bytes])),
                                                  resp_p.as_path(),
                                                  mt)));
                    }
                    Some(&(ref resp_p, false)) => {
                        return Ok(Response::with((status::Ok,
                                                  Header(headers::Server(USER_AGENT.to_string())),
                                                  Header(headers::LastModified(headers::HttpDate(file_time_modified(resp_p)))),
                                                  Header(headers::AcceptRanges(vec![headers::RangeUnit::Bytes])),
                                                  resp_p.as_path(),
                                                  mt)));
                    }
                    None => (),
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
                let gain = (file_length(&req_p.metadata().expect("Failed to get requested file metadata"), &req_p) as f64) /
                           (file_length(&resp_p.metadata().expect("Failed to get encoded file metadata"), &resp_p) as f64);
                if gain < MIN_ENCODING_GAIN {
                    let mut cache = self.cache_fs.write().expect("Filesystem cache write lock poisoned");
                    cache.insert(cache_key, (req_p.clone(), false));
                    fs::remove_file(resp_p).expect("Failed to remove too big encoded file");
                } else {
                    log!("{} encoded as {} for {:.1}% ratio",
                         iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                         encoding,
                         gain * 100f64);

                    let mut cache = self.cache_fs.write().expect("Filesystem cache write lock poisoned");
                    cache.insert(cache_key, (resp_p.clone(), true));

                    return Ok(Response::with((status::Ok,
                                              Header(headers::Server(USER_AGENT.to_string())),
                                              Header(headers::ContentEncoding(vec![encoding])),
                                              Header(headers::AcceptRanges(vec![headers::RangeUnit::Bytes])),
                                              resp_p.as_path(),
                                              mt)));
                }
            } else {
                log!("{} failed to encode as {}, sending identity",
                     iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                     encoding);
            }
        }

        Ok(Response::with((status::Ok,
                           (Header(headers::Server(USER_AGENT.to_string())),
                            Header(headers::LastModified(headers::HttpDate(file_time_modified(&req_p)))),
                            Header(headers::AcceptRanges(vec![headers::RangeUnit::Bytes]))),
                           req_p.as_path(),
                           Header(headers::ContentLength(file_length(&req_p.metadata().expect("Failed to get requested file metadata"), &req_p))),
                           mt)))
    }

    fn handle_get_raw_fs_dir(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        log!("{green}{}{reset} was served metadata for directory {magenta}{}{reset}",
             req.remote_addr,
             req_p.display());
        self.handle_raw_fs_api_response(status::Ok,
                                        &FilesetData {
                                            writes_supported: self.writes_temp_dir.is_some(),
                                            is_root: req.url.path() == [""],
                                            is_file: false,
                                            files: req_p.read_dir()
                                                .expect("Failed to read requested directory")
                                                .map(|p| p.expect("Failed to iterate over requested directory"))
                                                .filter(|f| {
                    let fp = f.path();
                    let mut symlink = false;
                    !((!self.follow_symlinks &&
                       (|| {
                        symlink = is_symlink(&fp);
                        symlink
                    })()) ||
                      (self.follow_symlinks && self.sandbox_symlinks && symlink && !is_descendant_of(fp, &self.hosted_directory.1)))
                })
                                                .map(|f| {
                    let is_file = is_actually_file(&f.file_type().expect("Failed to get file type"));
                    if is_file {
                        get_raw_fs_metadata(f.path())
                    } else {
                        RawFileData {
                            mime_type: "text/directory".parse().unwrap(),
                            name: f.file_name().into_string().expect("Failed to get file name"),
                            last_modified: file_time_modified(&f.path()),
                            size: 0,
                            is_file: false,
                        }
                    }
                })
                                                .collect(),
                                        })
    }

    fn handle_get_dir(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        if self.check_indices {
            let mut idx = req_p.join("index");
            if let Some(e) = INDEX_EXTENSIONS.iter()
                .find(|e| {
                    idx.set_extension(e);
                    idx.exists() &&
                    ((!self.follow_symlinks || !self.sandbox_symlinks) ||
                     (self.follow_symlinks && self.sandbox_symlinks && is_descendant_of(&req_p, &self.hosted_directory.1)))
                }) {
                if req.url.path().pop() == Some("") {
                    let r = self.handle_get_file(req, idx);
                    log!("{} found index file for directory {magenta}{}{reset}",
                         iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                         req_p.display());
                    return r;
                } else {
                    return self.handle_get_dir_index_no_slash(req, e);
                }
            }
        }

        if client_mobile(&req.headers) {
            self.handle_get_mobile_dir_listing(req, req_p)
        } else {
            self.handle_get_dir_listing(req, req_p)
        }
    }

    fn handle_get_dir_index_no_slash(&self, req: &mut Request, idx_ext: &str) -> IronResult<Response> {
        let new_url = req.url.to_string() + "/";
        log!("Redirecting {green}{}{reset} to {yellow}{}{reset} - found index file {magenta}index.{}{reset}",
             req.remote_addr,
             new_url,
             idx_ext);

        // We redirect here because if we don't and serve the index right away funky shit happens.
        // Example:
        //   - Without following slash:
        //     https://cloud.githubusercontent.com/assets/6709544/21442017/9eb20d64-c89b-11e6-8c7b-888b5f70a403.png
        //   - With following slash:
        //     https://cloud.githubusercontent.com/assets/6709544/21442028/a50918c4-c89b-11e6-8936-c29896947f6a.png
        Ok(Response::with((status::MovedPermanently, Header(headers::Server(USER_AGENT.to_string())), Header(headers::Location(new_url)))))
    }

    fn handle_get_mobile_dir_listing(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let relpath = (url_path(&req.url) + "/").replace("//", "/");
        let is_root = req.url.path() == [""];
        log!("{green}{}{reset} was served mobile directory listing for {magenta}{}{reset}",
             req.remote_addr,
             req_p.display());

        let parent_s = if is_root {
            String::new()
        } else {
            let rel_noslash = &relpath[0..relpath.len() - 1];
            let slash_idx = rel_noslash.rfind('/');
            format!("<a href=\"/{up_path}{up_path_slash}\" class=\"list entry top\"><span class=\"back_arrow_icon\">Parent directory</span></a> \
                     <a href=\"/{up_path}{up_path_slash}\" class=\"list entry bottom\"><span class=\"marker\">@</span>\
                       <span class=\"datetime\">{} UTC</span></a>",
                    file_time_modified(req_p.parent()
                            .expect("Failed to get requested directory's parent directory"))
                        .strftime("%F %T")
                        .unwrap(),
                    up_path = slash_idx.map(|i| &rel_noslash[0..i]).unwrap_or(""),
                    up_path_slash = if slash_idx.is_some() { "/" } else { "" })
        };
        let list_s = req_p.read_dir()
            .expect("Failed to read requested directory")
            .map(|p| p.expect("Failed to iterate over requested directory"))
            .filter(|f| {
                let fp = f.path();
                let mut symlink = false;
                !((!self.follow_symlinks &&
                   (|| {
                    symlink = is_symlink(&fp);
                    symlink
                })()) || (self.follow_symlinks && self.sandbox_symlinks && symlink && !is_descendant_of(fp, &self.hosted_directory.1)))
            })
            .sorted_by(|lhs, rhs| {
                (is_actually_file(&lhs.file_type().expect("Failed to get file type")),
                 lhs.file_name().to_str().expect("Failed to get file name").to_lowercase())
                    .cmp(&(is_actually_file(&rhs.file_type().expect("Failed to get file type")),
                           rhs.file_name().to_str().expect("Failed to get file name").to_lowercase()))
            })
            .fold("".to_string(), |cur, f| {
                let is_file = is_actually_file(&f.file_type().expect("Failed to get file type"));
                let fname = f.file_name().into_string().expect("Failed to get file name");
                let path = f.path();

                format!("{}<a href=\"{path}{fname}\" class=\"list entry top\"><span class=\"{}{}_icon\" id=\"{}\">{}{}</span></a> \
                         <a href=\"{path}{fname}\" class=\"list entry bottom\"><span class=\"marker\">@</span><span class=\"datetime\">{} UTC</span> \
                         {}{}{}</a>\n",
                        cur,
                        if is_file { "file" } else { "dir" },
                        file_icon_suffix(&path, is_file),
                        path.file_name().map(|p| p.to_str().expect("Filename not UTF-8").replace('.', "_")).as_ref().unwrap_or(&fname),
                        fname,
                        if is_file { "" } else { "/" },
                        file_time_modified(&path).strftime("%F %T").unwrap(),
                        if is_file { "<span class=\"size\">" } else { "" },
                        if is_file {
                            human_readable_size(file_length(&f.metadata().expect("Failed to get requested file metadata"), &path))
                        } else {
                            String::new()
                        },
                        if is_file { "</span>" } else { "" },
                        path = format!("/{}", relpath).replace("//", "/").replace('%', "%25").replace('#', "%23"),
                        fname = fname.replace('%', "%25").replace('#', "%23"))
            });

        self.handle_generated_response_encoding(req,
                                                status::Ok,
                                                html_response(MOBILE_DIRECTORY_LISTING_HTML,
                                                              &[&relpath[..],
                                                                if is_root { "" } else { "/" },
                                                                if self.writes_temp_dir.is_some() {
                                                                    r#"<script type="text/javascript">{upload}</script>"#
                                                                } else {
                                                                    ""
                                                                },
                                                                &parent_s[..],
                                                                &list_s[..],
                                                                if self.writes_temp_dir.is_some() {
                                                                    "<span class=\"list heading top top-border bottom\"> \
                                                                       Upload files: <input id=\"file_upload\" type=\"file\" multiple /> \
                                                                     </span>"
                                                                } else {
                                                                    ""
                                                                }]))
    }

    fn handle_get_dir_listing(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let relpath = (url_path(&req.url) + "/").replace("//", "/");
        let is_root = req.url.path() == [""];
        log!("{green}{}{reset} was served directory listing for {magenta}{}{reset}",
             req.remote_addr,
             req_p.display());

        let parent_s = if is_root {
            String::new()
        } else {
            let rel_noslash = &relpath[0..relpath.len() - 1];
            let slash_idx = rel_noslash.rfind('/');
            format!("<tr><td><a href=\"/{up_path}{up_path_slash}\" id=\"parent_dir\" class=\"back_arrow_icon\"></a></td> \
                         <td><a href=\"/{up_path}{up_path_slash}\">Parent directory</a></td> \
                         <td><a href=\"/{up_path}{up_path_slash}\" class=\"datetime\">{}</a></td> \
                         <td><a href=\"/{up_path}{up_path_slash}\">&nbsp;</a></td></tr>",
                    file_time_modified(req_p.parent().expect("Failed to get requested directory's parent directory")).strftime("%F %T").unwrap(),
                    up_path = slash_idx.map(|i| &rel_noslash[0..i]).unwrap_or(""),
                    up_path_slash = if slash_idx.is_some() { "/" } else { "" })
        };
        let list_s = req_p.read_dir()
            .expect("Failed to read requested directory")
            .map(|p| p.expect("Failed to iterate over requested directory"))
            .filter(|f| {
                let fp = f.path();
                let mut symlink = false;
                !((!self.follow_symlinks &&
                   (|| {
                    symlink = is_symlink(&fp);
                    symlink
                })()) || (self.follow_symlinks && self.sandbox_symlinks && symlink && !is_descendant_of(fp, &self.hosted_directory.1)))
            })
            .sorted_by(|lhs, rhs| {
                (is_actually_file(&lhs.file_type().expect("Failed to get file type")),
                 lhs.file_name().to_str().expect("Failed to get file name").to_lowercase())
                    .cmp(&(is_actually_file(&rhs.file_type().expect("Failed to get file type")),
                           rhs.file_name().to_str().expect("Failed to get file name").to_lowercase()))
            })
            .fold("".to_string(), |cur, f| {
                let is_file = is_actually_file(&f.file_type().expect("Failed to get file type"));
                let fname = f.file_name().into_string().expect("Failed to get file name");
                let path = f.path();
                let len = file_length(&f.metadata().expect("Failed to get requested file metadata"), &path);

                format!("{}<tr><td><a href=\"{path}{fname}\" id=\"{}\" class=\"{}{}_icon\"></a></td> \
                           <td><a href=\"{path}{fname}\">{}{}</a></td> <td><a href=\"{path}{fname}\" class=\"datetime\">{}</a></td> \
                           <td><a href=\"{path}{fname}\">{}{}{}{}{}</a></td></tr>\n",
                        cur,
                        path.file_name().map(|p| p.to_str().expect("Filename not UTF-8").replace('.', "_")).as_ref().unwrap_or(&fname),
                        if is_file { "file" } else { "dir" },
                        file_icon_suffix(&path, is_file),
                        fname,
                        if is_file { "" } else { "/" },
                        file_time_modified(&path).strftime("%F %T").unwrap(),
                        if is_file { "<abbr title=\"" } else { "&nbsp;" },
                        if is_file {
                            len.to_string()
                        } else {
                            String::new()
                        },
                        if is_file { " B\">" } else { "" },
                        if is_file {
                            human_readable_size(len)
                        } else {
                            String::new()
                        },
                        if is_file { "</abbr>" } else { "" },
                        path = format!("/{}", relpath).replace("//", "/").replace('%', "%25").replace('#', "%23"),
                        fname = fname.replace('%', "%25").replace('#', "%23"))
            });

        self.handle_generated_response_encoding(req,
                                                status::Ok,
                                                html_response(DIRECTORY_LISTING_HTML,
                                                              &[&relpath[..],
                                                                if self.writes_temp_dir.is_some() {
                                                                    r#"<script type="text/javascript">{upload}</script>"#
                                                                } else {
                                                                    ""
                                                                },
                                                                &parent_s[..],
                                                                &list_s[..],
                                                                if self.writes_temp_dir.is_some() {
                                                                    "<hr /> \
                                                                     <p> \
                                                                       Drag&amp;Drop to upload or <input id=\"file_upload\" type=\"file\" multiple />. \
                                                                     </p>"
                                                                } else {
                                                                    ""
                                                                }]))
    }

    fn handle_put(&self, req: &mut Request) -> IronResult<Response> {
        if self.writes_temp_dir.is_none() {
            return self.handle_forbidden_method(req, "-w", "write requests");
        }

        let (req_p, symlink, url_err) = self.parse_requested_path(req);

        if url_err {
            self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>")
        } else if req_p.is_dir() {
            self.handle_disallowed_method(req, &[method::Options, method::Get, method::Delete, method::Head, method::Trace], "directory")
        } else if detect_file_as_dir(&req_p) {
            self.handle_invalid_url(req, "<p>Attempted to use file as directory.</p>")
        } else if req.headers.has::<headers::ContentRange>() {
            self.handle_put_partial_content(req)
        } else if (symlink && !self.follow_symlinks) ||
                  (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_nonexistant_descendant_of(&req_p, &self.hosted_directory.1)) {
            self.create_temp_dir(&self.writes_temp_dir);
            self.handle_put_file(req, req_p, false)
        } else {
            self.create_temp_dir(&self.writes_temp_dir);
            self.handle_put_file(req, req_p, true)
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

        log!("{green}{}{reset} tried to {red}{}{reset} on {magenta}{}{reset} ({blue}{}{reset}) but only {red}{}{reset} are allowed",
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
        log!("{green}{}{reset} tried to {red}PUT{reset} partial content to {yellow}{}{reset}",
             req.remote_addr,
             url_path(&req.url));

        self.handle_generated_response_encoding(req,
                                                status::BadRequest,
                                                html_response(ERROR_HTML,
                                                              &["400 Bad Request",
                                                                "<a href=\"https://tools.ietf.org/html/rfc7231#section-4.3.3\">RFC7231 forbids \
                                                                 partial-content PUT requests.</a>",
                                                                ""]))
    }

    fn handle_put_file(&self, req: &mut Request, req_p: PathBuf, legal: bool) -> IronResult<Response> {
        let existant = !legal || req_p.exists();
        log!("{green}{}{reset} {} {magenta}{}{reset}, size: {}B",
             req.remote_addr,
             if !legal {
                 "tried to illegally create"
             } else if existant {
                 "replaced"
             } else {
                 "created"
             },
             req_p.display(),
             *req.headers.get::<headers::ContentLength>().expect("No Content-Length header"));

        let &(_, ref temp_dir) = self.writes_temp_dir.as_ref().unwrap();
        let temp_file_p = temp_dir.join(req_p.file_name().expect("Failed to get requested file's filename"));

        io::copy(&mut req.body, &mut File::create(&temp_file_p).expect("Failed to create temp file"))
            .expect("Failed to write requested data to requested file");
        if legal {
            let _ = fs::create_dir_all(req_p.parent().expect("Failed to get requested file's parent directory"));
            fs::copy(&temp_file_p, req_p).expect("Failed to copy temp file to requested file");
        }

        Ok(Response::with((if !legal || !existant {
                               status::Created
                           } else {
                               status::NoContent
                           },
                           Header(headers::Server(USER_AGENT.to_string())))))
    }

    fn handle_delete(&self, req: &mut Request) -> IronResult<Response> {
        if self.writes_temp_dir.is_none() {
            return self.handle_forbidden_method(req, "-w", "write requests");
        }

        let (req_p, symlink, url_err) = self.parse_requested_path_custom_symlink(req, false);

        if url_err {
            self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>")
        } else if !req_p.exists() || (symlink && !self.follow_symlinks) ||
                  (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&req_p, &self.hosted_directory.1)) {
            self.handle_nonexistant(req, req_p)
        } else {
            self.handle_delete_path(req, req_p, symlink)
        }
    }

    fn handle_delete_path(&self, req: &mut Request, req_p: PathBuf, symlink: bool) -> IronResult<Response> {
        log!("{green}{}{reset} deleted {blue}{} {magenta}{}{reset}",
             req.remote_addr,
             if is_actually_file(&req_p.metadata().expect("failed to get file metadata").file_type()) {
                 "file"
             } else if symlink {
                 "symlink"
             } else {
                 "directory"
             },
             req_p.display());

        if is_actually_file(&req_p.metadata().expect("failed to get file metadata").file_type()) {
            fs::remove_file(req_p).expect("Failed to remove requested file");
        } else {
            fs::remove_dir_all(req_p).expect(if symlink {
                "Failed to remove requested symlink"
            } else {
                "Failed to remove requested directory"
            });
        }

        Ok(Response::with((status::NoContent, Header(headers::Server(USER_AGENT.to_string())))))
    }

    fn handle_trace(&self, req: &mut Request) -> IronResult<Response> {
        log!("{green}{}{reset} requested {red}TRACE{reset} for {magenta}{}{reset}",
             req.remote_addr,
             url_path(&req.url));

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
        log!("{green}{}{reset} used disabled request method {red}{}{reset} grouped under {}",
             req.remote_addr,
             req.method,
             desc);

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
        log!("{green}{}{reset} used invalid request method {red}{}{reset}", req.remote_addr, req.method);

        let last_p = format!("<p>Unsupported request method: {}.<br />\nSupported methods: OPTIONS, GET, PUT, DELETE, HEAD and TRACE.</p>",
                             req.method);
        self.handle_generated_response_encoding(req,
                                                status::NotImplemented,
                                                html_response(ERROR_HTML, &["501 Not Implemented", "This operation was not implemented.", &last_p]))
    }

    fn handle_generated_response_encoding(&self, req: &mut Request, st: status::Status, resp: String) -> IronResult<Response> {
        if let Some(encoding) = req.headers.get_mut::<headers::AcceptEncoding>().and_then(|es| response_encoding(&mut **es)) {
            let mut cache_key = ([0u8; 32], encoding.to_string());
            md6::hash(256, resp.as_bytes(), &mut cache_key.0).expect("Failed to hash generated response");

            {
                if let Some(enc_resp) = self.cache_gen.read().expect("Generated file cache read lock poisoned").get(&cache_key) {
                    log!("{} encoded as {} for {:.1}% ratio (cached)",
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
                log!("{} encoded as {} for {:.1}% ratio",
                     iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                     encoding,
                     ((resp.len() as f64) / (enc_resp.len() as f64)) * 100f64);

                let mut cache = self.cache_gen.write().expect("Generated file cache read lock poisoned");
                cache.insert(cache_key.clone(), enc_resp);

                return Ok(Response::with((st,
                                          Header(headers::Server(USER_AGENT.to_string())),
                                          Header(headers::ContentEncoding(vec![encoding])),
                                          "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                                          &cache[&cache_key][..])));
            } else {
                log!("{} failed to encode as {}, sending identity",
                     iter::repeat(' ').take(req.remote_addr.to_string().len()).collect::<String>(),
                     encoding);
            }
        }

        Ok(Response::with((st, Header(headers::Server(USER_AGENT.to_string())), "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(), resp)))
    }

    fn handle_raw_fs_api_response<R: Serialize>(&self, st: status::Status, resp: &R) -> IronResult<Response> {
        Ok(Response::with((st,
                           Header(headers::Server(USER_AGENT.to_string())),
                           Header(RawFsApiHeader(true)),
                           "application/json;charset=utf-8".parse::<mime::Mime>().unwrap(),
                           serde_json::to_string(&resp).unwrap())))
    }

    fn parse_requested_path(&self, req: &Request) -> (PathBuf, bool, bool) {
        self.parse_requested_path_custom_symlink(req, true)
    }

    fn parse_requested_path_custom_symlink(&self, req: &Request, follow_symlinks: bool) -> (PathBuf, bool, bool) {
        req.url.path().into_iter().filter(|p| !p.is_empty()).fold((self.hosted_directory.1.clone(), false, false), |(mut cur, mut sk, mut err), pp| {
            if let Some(pp) = percent_decode(pp) {
                cur.push(&*pp);
            } else {
                err = true;
            }
            while let Ok(newlink) = cur.read_link() {
                sk = true;
                if follow_symlinks {
                    cur = newlink;
                } else {
                    break;
                }
            }
            (cur, sk, err)
        })
    }

    fn create_temp_dir(&self, td: &Option<(String, PathBuf)>) {
        let &(ref temp_name, ref temp_dir) = td.as_ref().unwrap();
        if !temp_dir.exists() && fs::create_dir_all(&temp_dir).is_ok() {
            log!("Created temp dir {magenta}{}{reset}", temp_name);
        }
    }
}

impl Clone for HttpHandler {
    fn clone(&self) -> HttpHandler {
        HttpHandler {
            hosted_directory: self.hosted_directory.clone(),
            follow_symlinks: self.follow_symlinks,
            sandbox_symlinks: self.sandbox_symlinks,
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
/// let server = try_ports(|req| Ok(Response::with((status::Ok, "Abolish the burgeoisie!"))), 8000, 8100, None).unwrap();
/// ```
pub fn try_ports<H: Handler + Clone>(hndlr: H, from: u16, up_to: u16, tls_data: &Option<((String, PathBuf), String)>) -> Result<Listening, Error> {
    let hndlr = hndlr;
    for port in from..up_to + 1 {
        let ir = Iron::new(hndlr.clone());
        match if let Some(&((_, ref id), ref pw)) = tls_data.as_ref() {
            ir.https(("0.0.0.0", port),
                     try!(NativeTlsServer::new(id, pw).map_err(|_| {
                Error::Io {
                    desc: "TLS certificate",
                    op: "open",
                    more: None,
                }
            })))
        } else {
            ir.http(("0.0.0.0", port))
        } {
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
        more: Some("no free ports".into()),
    })
}

/// Generate a passwordless self-signed certificate in the `"tls"` subdirectory of the specified directory
/// with the filenames `"tls.*"`.
///
/// # Examples
///
/// ```
/// # use https::ops::generate_tls_data;
/// let ((ident_name, ident_file), pass) = generate_tls_data(&(".".to_string(), ".".into())).unwrap();
/// assert_eq!(ident_name, "./tls/tls.p12");
/// assert!(ident_file.exists());
/// assert_eq!(pass, "");
/// ```
pub fn generate_tls_data(temp_dir: &(String, PathBuf)) -> Result<((String, PathBuf), String), Error> {
    fn err(which: bool, op: &'static str, more: Option<Cow<'static, str>>) -> Error {
        Error::Io {
            desc: if which {
                "TLS key generation process"
            } else {
                "TLS identity generation process"
            },
            op: op,
            more: more,
        }
    }
    fn exit_err(which: bool, process: &mut Child, exitc: &ExitStatus) -> Error {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if process.stdout.as_mut().unwrap().read_to_string(&mut stdout).is_err() {
            stdout = "<error getting process stdout".to_string();
        }
        if process.stderr.as_mut().unwrap().read_to_string(&mut stderr).is_err() {
            stderr = "<error getting process stderr".to_string();
        }

        err(which,
            "exit",
            Some(format!("{};\nstdout: ```\n{}```;\nstderr: ```\n{}```", exitc, stdout, stderr).into()))
    }

    let tls_dir = temp_dir.1.join("tls");
    if !tls_dir.exists() && fs::create_dir_all(&tls_dir).is_err() {
        return Err(Error::Io {
            desc: "temporary directory",
            op: "create",
            more: None,
        });
    }

    let mut child = try!(Command::new("openssl")
        .args(&["req", "-x509", "-newkey", "rsa:4096", "-nodes", "-keyout", "tls.key", "-out", "tls.crt", "-days", "3650", "-utf8"])
        .current_dir(&tls_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| err(true, "spawn", None)));
    try!(child.stdin
        .as_mut()
        .unwrap()
        .write_all(concat!("PL\nhttp\n",
                           env!("CARGO_PKG_VERSION"),
                           "\nthecoshman&nabijaczleweli\nнаб\nhttp/",
                           env!("CARGO_PKG_VERSION"),
                           "\nnabijaczleweli@gmail.com\n")
            .as_bytes())
        .map_err(|_| err(true, "pipe", None)));
    let es = try!(child.wait().map_err(|_| err(true, "wait", None)));
    if !es.success() {
        return Err(exit_err(true, &mut child, &es));
    }

    let mut child = try!(Command::new("openssl")
        .args(&["pkcs12", "-export", "-out", "tls.p12", "-inkey", "tls.key", "-in", "tls.crt", "-passin", "pass:", "-passout", "pass:"])
        .current_dir(&tls_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| err(false, "spawn", None)));
    let es = try!(child.wait().map_err(|_| err(false, "wait", None)));
    if !es.success() {
        return Err(exit_err(false, &mut child, &es));
    }

    Ok(((format!("{}/tls/tls.p12", temp_dir.0), tls_dir.join("tls.p12")), String::new()))
}
