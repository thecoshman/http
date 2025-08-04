use blake3;
use serde_json;
use std::net::IpAddr;
use serde::Serialize;
use std::sync::RwLock;
use std::{fmt, str, mem};
use cidr::{Cidr, IpCidr};
use std::fs::{self, File};
use arrayvec::ArrayString;
use std::default::Default;
use iron::modifiers::Header;
use std::path::{PathBuf, Path};
use std::ffi::{OsString, OsStr};
use std::fmt::Write as FmtWrite;
use iron::headers::EncodingType;
use iron::url::Url as GenericUrl;
use mime_guess::get_mime_type_opt;
use hyper_native_tls::NativeTlsServer;
use std::hash::{BuildHasher, RandomState};
use std::collections::{BTreeMap, HashMap};
use self::super::{LogLevel, Options, Error};
use std::process::{ExitStatus, Command, Child, Stdio};
use rfsapi::{RawFsApiHeader, FilesetData, RawFileData};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use iron::{headers, status, method, IronResult, Listening, Response, Headers, Request, Handler, Iron};
use std::io::{self, ErrorKind as IoErrorKind, BufReader, SeekFrom, Write, Error as IoError, Read, Seek};
use iron::mime::{Mime, Attr as MimeAttr, Value as MimeAttrValue, SubLevel as MimeSubLevel, TopLevel as MimeTopLevel};
use self::super::util::{HumanReadableSize, WwwAuthenticate, NoDoubleQuotes, NoHtmlLiteral, XLastModified, DisplayThree, CommaList, XOcMTime, MsAsS, Maybe, Dav,
                        url_path, file_etag, file_hash, set_mtime_f, is_symlink, encode_str, error_html, encode_file, file_length, file_binary, client_mobile,
                        percent_decode, escape_specials, precise_time_ns, file_icon_suffix, is_actually_file, is_descendant_of, response_encoding,
                        detect_file_as_dir, encoding_extension, file_time_modified, file_time_modified_p, dav_level_1_methods, get_raw_fs_metadata,
                        encode_tail_if_trimmed, extension_is_blacklisted, directory_listing_html, directory_listing_mobile_html, is_nonexistent_descendant_of,
                        USER_AGENT, MAX_SYMLINKS, INDEX_EXTENSIONS, MIN_ENCODING_GAIN, MAX_ENCODING_SIZE, MIN_ENCODING_SIZE};

macro_rules! log {
    ($logcfg:expr, $fmt:expr) => {
        use chrono::Local;
        use trivial_colours::{Reset as CReset, Colour as C};

        if $logcfg.0 {
            if $logcfg.2 {
                if $logcfg.1 {
                    print!("{}[{}]{} ", C::Cyan, Local::now().format("%F %T"), CReset);
                }
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
            } else {
                if $logcfg.1 {
                    print!("[{}] ", Local::now().format("%F %T"));
                }
                println!(concat!($fmt, "{black:.0}{red:.0}{green:.0}{yellow:.0}{blue:.0}{magenta:.0}{cyan:.0}{white:.0}{reset:.0}"),
                         black = "",
                         red = "",
                         green = "",
                         yellow = "",
                         blue = "",
                         magenta = "",
                         cyan = "",
                         white = "",
                         reset = "");
            }
        }
    };
    ($logcfg:expr, $fmt:expr, $($arg:tt)*) => {
        use chrono::Local;
        use trivial_colours::{Reset as CReset, Colour as C};

        if $logcfg.0 {
            if $logcfg.2 {
                if $logcfg.1 {
                    print!("{}[{}]{} ", C::Cyan, Local::now().format("%F %T"), CReset);
                }
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
            } else {
                if $logcfg.1 {
                    print!("[{}] ", Local::now().format("%F %T"));
                }
                println!(concat!($fmt, "{black:.0}{red:.0}{green:.0}{yellow:.0}{blue:.0}{magenta:.0}{cyan:.0}{white:.0}{reset:.0}"),
                         $($arg)*,
                         black = "",
                         red = "",
                         green = "",
                         yellow = "",
                         blue = "",
                         magenta = "",
                         cyan = "",
                         white = "",
                         reset = "");
            }
        }
    };
}

mod prune;
mod webdav;
mod bandwidth;

pub use self::prune::PruneChain;
pub use self::bandwidth::{LimitBandwidthMiddleware, SimpleChain};


type CacheT<Cnt> = HashMap<(blake3::Hash, EncodingType), (Cnt, AtomicU64)>;

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum WebDavLevel {
    No,
    MkColMoveOnly,
    All,
}

pub struct HttpHandler {
    pub hosted_directory: (String, PathBuf),
    pub follow_symlinks: bool,
    pub sandbox_symlinks: bool,
    pub generate_listings: bool,
    pub check_indices: bool,
    pub strip_extensions: bool,
    pub try_404: Option<PathBuf>,
    /// (at all, log_time, log_colour)
    pub log: (bool, bool, bool),
    pub webdav: WebDavLevel,
    pub global_auth_data: Option<(String, Option<String>)>,
    pub path_auth_data: BTreeMap<String, Option<(String, Option<String>)>>,
    pub writes_temp_dir: Option<(String, PathBuf)>,
    pub encoded_temp_dir: Option<(String, PathBuf)>,
    pub proxies: BTreeMap<IpCidr, String>,
    pub proxy_redirs: BTreeMap<IpCidr, String>,
    pub mime_type_overrides: BTreeMap<OsString, Mime>,
    pub additional_headers: Vec<(String, Vec<u8>)>,

    pub cache_gen: RwLock<CacheT<Vec<u8>>>,
    pub cache_fs_files: RwLock<HashMap<String, blake3::Hash>>, // etag -> cache key
    pub cache_fs: RwLock<CacheT<(PathBuf, bool, u64)>>,
    pub cache_gen_size: AtomicU64,
    pub cache_fs_size: AtomicU64,
    pub encoded_filesystem_limit: u64,
    pub encoded_generated_limit: u64,

    pub allowed_methods: &'static [method::Method],
}

impl HttpHandler {
    pub fn new(opts: &Options) -> HttpHandler {
        let mut path_auth_data = BTreeMap::new();
        let mut global_auth_data = None;

        for (path, creds) in &opts.path_auth_data {
            let creds = creds.as_ref()
                .map(|auth| {
                    let mut itr = auth.split_terminator(':');
                    (itr.next().unwrap().to_string(), itr.next().map(str::to_string))
                });

            if path == "" {
                global_auth_data = creds;
            } else {
                path_auth_data.insert(path.to_string(), creds);
            }
        }

        let allowed_methods = [method::Options, method::Get, method::Head, method::Trace]
            .iter()
            .chain(dav_level_1_methods(opts.allow_writes)
                .iter()
                .filter(|method| {
                    opts.webdav == WebDavLevel::All || (opts.webdav == WebDavLevel::MkColMoveOnly && matches!(**method, method::DavMkcol | method::DavMove))
                }))
            .chain([method::Put, method::Delete].iter().filter(|_| opts.allow_writes))
            .cloned()
            .collect::<Vec<_>>()
            .leak();

        HttpHandler {
            hosted_directory: opts.hosted_directory.clone(),
            follow_symlinks: opts.follow_symlinks,
            sandbox_symlinks: opts.sandbox_symlinks,
            generate_listings: opts.generate_listings,
            check_indices: opts.check_indices,
            strip_extensions: opts.strip_extensions,
            try_404: opts.try_404.clone(),
            log: (opts.loglevel < LogLevel::NoServeStatus, opts.log_time, opts.log_colour),
            webdav: opts.webdav,
            global_auth_data: global_auth_data,
            path_auth_data: path_auth_data,
            writes_temp_dir: HttpHandler::temp_subdir(&opts.temp_directory, opts.allow_writes, "writes"),
            encoded_temp_dir: HttpHandler::temp_subdir(&opts.temp_directory, opts.encode_fs, "encoded"),
            cache_gen: Default::default(),
            cache_fs: Default::default(),
            cache_fs_files: Default::default(),
            cache_gen_size: Default::default(),
            cache_fs_size: Default::default(),
            encoded_filesystem_limit: opts.encoded_filesystem_limit.unwrap_or(u64::MAX),
            encoded_generated_limit: opts.encoded_generated_limit.unwrap_or(u64::MAX),
            proxies: opts.proxies.clone(),
            proxy_redirs: opts.proxy_redirs.clone(),
            mime_type_overrides: opts.mime_type_overrides.clone(),
            additional_headers: opts.additional_headers.clone(),
            allowed_methods: allowed_methods,
        }
    }

    pub fn clean_temp_dirs(&self, temp_directory: &(String, PathBuf), generate_tls: bool) {
        mem::forget(self.cache_fs_files.write());
        mem::forget(self.cache_fs.write());

        let tls = HttpHandler::temp_subdir(temp_directory, generate_tls, "tls");
        for (temp_name, temp_dir) in [self.writes_temp_dir.as_ref(), self.encoded_temp_dir.as_ref(), tls.as_ref()].iter().flatten() {
            if fs::remove_dir_all(&temp_dir).is_ok() {
                log!(self.log, "Deleted temp dir {magenta}{}{reset}", temp_name);
            }
        }
        if fs::remove_dir(&temp_directory.1).is_ok() {
            log!(self.log, "Deleted temp dir {magenta}{}{reset}", temp_directory.0);
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

impl Handler for &'static HttpHandler {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        if self.global_auth_data.is_some() || !self.path_auth_data.is_empty() {
            if let Some(resp) = self.verify_auth(req)? {
                return Ok(resp);
            }
        }

        let mut resp = match req.method {
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

            method::DavCopy if self.webdav >= WebDavLevel::All => self.handle_webdav_copy(req),
            method::DavMkcol if self.webdav >= WebDavLevel::MkColMoveOnly => self.handle_webdav_mkcol(req),
            method::DavMove if self.webdav >= WebDavLevel::MkColMoveOnly => self.handle_webdav_move(req),
            method::DavPropfind if self.webdav >= WebDavLevel::All => self.handle_webdav_propfind(req),
            method::DavProppatch if self.webdav >= WebDavLevel::All => self.handle_webdav_proppatch(req),

            _ => self.handle_bad_method(req),
        }?;
        if self.webdav >= WebDavLevel::All {
            resp.headers.set(Dav::LEVEL_1);
        }
        for (h, v) in &self.additional_headers {
            resp.headers.append_raw(&h[..], v[..].into());
        }
        Ok(resp)
    }
}

impl HttpHandler {
    fn verify_auth(&self, req: &mut Request) -> IronResult<Option<Response>> {
        let mut auth = self.global_auth_data.as_ref();

        if !self.path_auth_data.is_empty() {
            let mut path = req.url.as_ref().path();
            if path.starts_with('/') {
                path = &path[1..];
            }
            if path.ends_with('/') {
                path = &path[..path.len() - 1];
            }

            while !path.is_empty() {
                if let Some(pad) = self.path_auth_data.get(path) {
                    auth = pad.as_ref();
                    break;
                }

                path = &path[..path.rfind('/').unwrap_or(0)];
            }
        }

        let auth = if let Some(auth) = auth {
            auth
        } else {
            return Ok(None);
        };

        match req.headers.get() {
            Some(headers::Authorization(headers::Basic { username, password })) => {
                let pwd = if password == &Some(String::new()) {
                    &None
                } else {
                    password
                };

                if &auth.0 == username && &auth.1 == pwd {
                    log!(self.log,
                         "{} correctly authorised to {red}{}{reset} {yellow}{}{reset}",
                         self.remote_addresses(&req),
                         req.method,
                         req.url);

                    Ok(None)
                } else {
                    log!(self.log,
                         "{} requested to {red}{}{reset} {yellow}{}{reset} with invalid credentials \"{}{}{}\"",
                         self.remote_addresses(&req),
                         req.method,
                         req.url,
                         username,
                         if password.is_some() { ":" } else { "" },
                         password.as_ref().map_or("", |s| &s[..]));

                    Ok(Some(Response::with((status::Unauthorized, Header(WwwAuthenticate("basic".into())), "Supplied credentials invalid.\n"))))
                }
            }
            None => {
                log!(self.log,
                     "{} requested to {red}{}{reset} {yellow}{}{reset} without authorisation",
                     self.remote_addresses(&req),
                     req.method,
                     req.url);

                Ok(Some(Response::with((status::Unauthorized, Header(WwwAuthenticate("basic".into())), "Credentials required.\n"))))
            }
        }
    }

    fn handle_options(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log, "{} asked for {red}OPTIONS{reset}", self.remote_addresses(&req));
        Ok(Response::with((status::NoContent, Header(headers::Server(USER_AGENT.into())), Header(headers::Allow(self.allowed_methods.into())))))
    }

    fn handle_get(&self, req: &mut Request) -> IronResult<Response> {
        let (mut req_p, symlink, url_err) = self.parse_requested_path(req);

        if url_err {
            return self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>");
        }

        if !req_p.exists() && req_p.extension().is_none() && self.strip_extensions {
            if let Some(rp) = INDEX_EXTENSIONS.iter().map(|ext| req_p.with_extension(ext)).find(|rp| rp.exists()) {
                req_p = rp;
            }
        }

        if !req_p.exists() || (symlink && !self.follow_symlinks) ||
           (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&req_p, &self.hosted_directory.1)) {
            return self.handle_nonexistent_get(req, req_p);
        }

        let is_file = is_actually_file(&req_p.metadata().expect("Failed to get file metadata").file_type(), &req_p);
        let range = req.headers.get_mut().map(|r: &mut headers::Range| mem::replace(r, headers::Range::Bytes(vec![])));
        let raw_fs = req.headers.get().map(|r: &RawFsApiHeader| r.0).unwrap_or(false);
        if is_file {
            if raw_fs {
                self.handle_get_raw_fs_file(req, req_p)
            } else if let Some(range) = range {
                self.handle_get_file_range(req, req_p, range)
            } else {
                self.handle_get_file(req, &req_p, false)
            }
        } else {
            if raw_fs {
                self.handle_get_raw_fs_dir(req, req_p)
            } else {
                self.handle_get_dir(req, req_p)
            }
        }
    }

    fn handle_invalid_url(&self, req: &mut Request, cause: &str) -> IronResult<Response> {
        log!(self.log,
             "{} requested to {red}{}{reset} {yellow}{}{reset} with invalid URL -- {}",
             self.remote_addresses(&req),
             req.method,
             req.url,
             &cause[3..cause.len() - 4]); // Strip <p> tags

        self.handle_generated_response_encoding(req, status::BadRequest, error_html("400 Bad Request", "The request URL was invalid.", cause))
    }

    #[inline(always)]
    fn handle_nonexistent(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        self.handle_nonexistent_status(req, req_p, status::NotFound)
    }

    fn handle_nonexistent_status(&self, req: &mut Request, req_p: PathBuf, status: status::Status) -> IronResult<Response> {
        self.handle_nonexistent_status_impl(req, req_p, status, &None)
    }

    fn handle_nonexistent_get(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        self.handle_nonexistent_status_impl(req, req_p, status::NotFound, &self.try_404)
    }

    fn handle_nonexistent_status_impl(&self, req: &mut Request, req_p: PathBuf, status: status::Status, try_404: &Option<PathBuf>) -> IronResult<Response> {
        log!(self.log,
             "{} requested to {red}{}{reset} nonexistent entity {magenta}{}{reset}",
             self.remote_addresses(&req),
             req.method,
             req_p.display());

        if let Some(try_404) = try_404.as_ref() {
            if try_404.metadata().map(|m| !m.is_dir()).unwrap_or(false) {
                return self.handle_get_file(req, try_404, true)
            }
        }

        let url_p = url_path(&req.url);
        self.handle_generated_response_encoding(req,
                                                status,
                                                error_html(&status.canonical_reason().unwrap()[..],
                                                           format_args!("The requested entity \"{}\" doesn't exist.", url_p),
                                                           ""))
    }

    fn handle_get_raw_fs_file(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        log!(self.log,
             "{} was served metadata for file {magenta}{}{reset}",
             self.remote_addresses(&req),
             req_p.display());
        self.handle_raw_fs_api_response(status::Ok,
                                        &FilesetData {
                                            writes_supported: self.writes_temp_dir.is_some(),
                                            is_root: false,
                                            is_file: true,
                                            files: vec![get_raw_fs_metadata(&req_p)],
                                        })
    }

    fn etag_match(req_tags: &[headers::EntityTag], etag: &str) -> bool {
        req_tags.iter().any(|retag| retag.tag() == etag)
    }

    fn should_304_path(req: &mut Request, req_p: &Path, etag: &str) -> bool {
        if let Some(headers::IfNoneMatch::Items(inm)) = req.headers.get::<headers::IfNoneMatch>() {
            if HttpHandler::etag_match(inm, &etag) {
                return true;
            }
        } else if let Some(headers::IfModifiedSince(since)) = req.headers.get::<headers::IfModifiedSince>() {
            if file_time_modified_p(req_p) <= since.0 {
                return true;
            }
        }

        return false;
    }

    fn handle_get_file_range(&self, req: &mut Request, req_p: PathBuf, range: headers::Range) -> IronResult<Response> {
        match range {
            headers::Range::Bytes(ref brs) => {
                if brs.len() == 1 {
                    let metadata = req_p.metadata().expect("Failed to get requested file metadata");
                    let flen = file_length(&metadata, &req_p);

                    let mut etag = file_etag(&metadata).into_bytes(); // normaletag+123-41231
                    let _ = write!(&mut etag, "+{}", brs[0]);
                    let etag = unsafe { String::from_utf8_unchecked(etag) };
                    if HttpHandler::should_304_path(req, &req_p, &etag) {
                        log!(self.log, "{} Not Modified", self.remote_addresses(req));
                        return Ok(Response::with((status::NotModified,
                                                  (Header(headers::Server(USER_AGENT.into())),
                                                   Header(headers::LastModified(headers::HttpDate(file_time_modified_p(&req_p).into()))),
                                                   Header(headers::AcceptRanges(headers::RangeUnit::Bytes))),
                                                  Header(headers::ETag(headers::EntityTag::strong(etag))))));
                    }

                    match brs[0] {
                        // Cases where from is bigger than to are filtered out by iron so can never happen
                        headers::ByteRangeSpec::FromTo(from, to) => self.handle_get_file_closed_range(req, req_p, from, to, etag),
                        headers::ByteRangeSpec::AllFrom(from) => {
                            if flen < from {
                                self.handle_get_file_empty_range(req, req_p, from, flen, etag)
                            } else {
                                self.handle_get_file_right_opened_range(req, req_p, from, etag)
                            }
                        }
                        headers::ByteRangeSpec::Last(from) => {
                            if flen < from {
                                self.handle_get_file_empty_range(req, req_p, from, flen, etag)
                            } else {
                                self.handle_get_file_left_opened_range(req, req_p, from, etag)
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

    fn handle_get_file_closed_range(&self, req: &mut Request, req_p: PathBuf, from: u64, to: u64, etag: String) -> IronResult<Response> {
        let mime_type = self.guess_mime_type(&req_p);
        log!(self.log,
             "{} was served byte range {}-{} of file {magenta}{}{reset} as {blue}{}{reset}",
             self.remote_addresses(&req),
             from,
             to,
             req_p.display(),
             mime_type);

        let mut f = File::open(&req_p).expect("Failed to open requested file");
        f.seek(SeekFrom::Start(from)).expect("Failed to seek requested file");

        Ok(Response::with((status::PartialContent,
                           (Header(headers::Server(USER_AGENT.into())),
                            Header(headers::LastModified(headers::HttpDate(file_time_modified_p(&req_p).into()))),
                            Header(headers::ContentRange(headers::ContentRangeSpec::Bytes {
                                range: Some((from, to)),
                                instance_length: Some(file_length(&f.metadata().expect("Failed to get requested file metadata"), &req_p)),
                            })),
                            Header(headers::ETag(headers::EntityTag::strong(etag))),
                            Header(headers::AcceptRanges(headers::RangeUnit::Bytes))),
                           f,
                           mime_type,
                           Header(headers::ContentLength(to + 1 - from)))))
    }

    fn handle_get_file_right_opened_range(&self, req: &mut Request, req_p: PathBuf, from: u64, etag: String) -> IronResult<Response> {
        let mime_type = self.guess_mime_type(&req_p);
        log!(self.log,
             "{} was served file {magenta}{}{reset} from byte {} as {blue}{}{reset}",
             self.remote_addresses(&req),
             req_p.display(),
             from,
             mime_type);

        self.handle_get_file_opened_range(req_p, |flen| (SeekFrom::Start(from), from, flen - from), mime_type, etag)
    }

    fn handle_get_file_left_opened_range(&self, req: &mut Request, req_p: PathBuf, from: u64, etag: String) -> IronResult<Response> {
        let mime_type = self.guess_mime_type(&req_p);
        log!(self.log,
             "{} was served last {} bytes of file {magenta}{}{reset} as {blue}{}{reset}",
             self.remote_addresses(&req),
             from,
             req_p.display(),
             mime_type);

        self.handle_get_file_opened_range(req_p, |flen| (SeekFrom::End(-(from as i64)), flen - from, from), mime_type, etag)
    }

    fn handle_get_file_opened_range<F: FnOnce(u64) -> (SeekFrom, u64, u64)>(&self, req_p: PathBuf, cb: F, mt: Mime, etag: String) -> IronResult<Response> {
        let mut f = File::open(&req_p).expect("Failed to open requested file");
        let fmeta = f.metadata().expect("Failed to get requested file metadata");
        let flen = file_length(&fmeta, &req_p);
        let (s, b_from, clen) = cb(flen);
        f.seek(s).expect("Failed to seek requested file");

        Ok(Response::with((status::PartialContent,
                           f,
                           (Header(headers::Server(USER_AGENT.into())),
                            Header(headers::LastModified(headers::HttpDate(file_time_modified(&fmeta).into()))),
                            Header(headers::ContentRange(headers::ContentRangeSpec::Bytes {
                                range: Some((b_from, flen - 1)),
                                instance_length: Some(flen),
                            })),
                            Header(headers::ETag(headers::EntityTag::strong(etag))),
                            Header(headers::ContentLength(clen)),
                            Header(headers::AcceptRanges(headers::RangeUnit::Bytes))),
                           mt)))
    }

    fn handle_invalid_range(&self, req: &mut Request, req_p: PathBuf, range: &headers::Range, reason: &str) -> IronResult<Response> {
        self.handle_generated_response_encoding(req,
                                                status::RangeNotSatisfiable,
                                                error_html("416 Range Not Satisfiable",
                                                           format_args!("Requested range <samp>{}</samp> could not be fulfilled for file {}.",
                                                                        range,
                                                                        req_p.display()),
                                                           reason))
    }

    fn handle_get_file_empty_range(&self, req: &mut Request, req_p: PathBuf, from: u64, to: u64, etag: String) -> IronResult<Response> {
        let mime_type = self.guess_mime_type(&req_p);
        log!(self.log,
             "{} was served an empty range from file {magenta}{}{reset} as {blue}{}{reset}",
             self.remote_addresses(&req),
             req_p.display(),
             mime_type);

        Ok(Response::with((status::NoContent,
                           (Header(headers::Server(USER_AGENT.into())),
                            Header(headers::LastModified(headers::HttpDate(file_time_modified_p(&req_p).into()))),
                            Header(headers::ContentRange(headers::ContentRangeSpec::Bytes {
                                range: Some((from, to)),
                                instance_length: Some(file_length(&req_p.metadata().expect("Failed to get requested file metadata"), &req_p)),
                            }))),
                           Header(headers::ETag(headers::EntityTag::strong(etag))),
                           Header(headers::AcceptRanges(headers::RangeUnit::Bytes)),
                           mime_type)))
    }

    fn handle_get_file(&self, req: &mut Request, req_p: &PathBuf, is_404: bool) -> IronResult<Response> {
        let mime_type = self.guess_mime_type(&req_p);
        log!(self.log,
             "{} was served file {magenta}{}{reset} as {blue}{}{reset}",
             self.remote_addresses(&req).maybe_spaces(is_404),
             req_p.display(),
             mime_type);

        let metadata = &req_p.metadata().expect("Failed to get requested file metadata");
        let etag = file_etag(&metadata);
        let headers = (Header(headers::Server(USER_AGENT.into())),
                       Header(headers::LastModified(headers::HttpDate(file_time_modified(&metadata).into()))),
                       Header(headers::AcceptRanges(headers::RangeUnit::Bytes)));
        if HttpHandler::should_304_path(req, &req_p, &etag) {
            log!(self.log, "{} Not Modified", self.remote_addresses(req).as_spaces());
            return Ok(Response::with((status::NotModified, headers, Header(headers::ETag(headers::EntityTag::strong(etag))))));
        }

        let flen = file_length(&metadata, &req_p);
        if self.encoded_temp_dir.is_some() && flen > MIN_ENCODING_SIZE && flen < MAX_ENCODING_SIZE &&
           req_p.extension().map(|s| !extension_is_blacklisted(s)).unwrap_or(true) {
            self.handle_get_file_encoded(req, &req_p, mime_type, headers, etag)
        } else {
            let file = match File::open(&req_p) {
                Ok(file) => file,
                Err(err) => return self.handle_requested_entity_unopenable(req, err, "file"),
            };
            Ok(Response::with((if is_404 { status::NotFound } else { status::Ok },
                               headers,
                               Header(headers::ETag(headers::EntityTag::strong(etag))),
                               file,
                               mime_type,
                               Header(headers::ContentLength(file_length(&metadata, &req_p))))))
        }
    }

    fn handle_get_file_encoded(&self, req: &mut Request, req_p: &PathBuf, mt: Mime,
                               headers: (Header<headers::Server>, Header<headers::LastModified>, Header<headers::AcceptRanges>), etag: String)
                               -> IronResult<Response> {
        if let Some(encoding) = req.headers.get_mut::<headers::AcceptEncoding>().and_then(|es| response_encoding(&mut **es)) {
            self.create_temp_dir(&self.encoded_temp_dir);

            let hash = self.cache_fs_files.read().expect("Filesystem file cache read lock poisoned").get(&etag).cloned();
            let hash = match hash {
                Some(hash) => hash,
                None => {
                    match file_hash(&req_p) {
                        Ok(h) => {
                            self.cache_fs_files.write().expect("Filesystem file cache write lock poisoned").insert(etag.clone(), h);
                            h
                        }
                        Err(err) => return self.handle_requested_entity_unopenable(req, err, "file"),
                    }
                }
            };
            let cache_key = (hash, encoding.0);

            let forgor = {
                match self.cache_fs.read().expect("Filesystem cache read lock poisoned").get(&cache_key) {
                    Some(&((ref resp_p, true, _), ref atime)) => {
                        match File::open(resp_p) {
                            Ok(resp) => {
                                atime.store(precise_time_ns(), AtomicOrdering::Relaxed);
                                log!(self.log,
                                     "{} encoded as {} for {:.1}% ratio (cached)",
                                     self.remote_addresses(req).as_spaces(),
                                     encoding,
                                     ((file_length(&req_p.metadata().expect("Failed to get requested file metadata"), &req_p) as f64) /
                                      (file_length(&resp.metadata().expect("Failed to get encoded file metadata"), &resp_p) as f64)) *
                                     100f64);

                                return Ok(Response::with((status::Ok,
                                                          headers,
                                                          Header(headers::ETag(headers::EntityTag::strong(etag))),
                                                          Header(headers::ContentEncoding([encoding].into())),
                                                          resp,
                                                          mt)));
                            }
                            Err(err) if err.kind() == IoErrorKind::NotFound => true,
                            e @ Err(_) => {
                                e.expect("Failed to get encoded file metadata");
                                unsafe { std::hint::unreachable_unchecked() }
                            }
                        }
                    }
                    Some(&((_, false, _), _)) => {
                        let file = match File::open(&req_p) {
                            Ok(file) => file,
                            Err(err) => return self.handle_requested_entity_unopenable(req, err, "file"),
                        };
                        return Ok(Response::with((status::Ok, headers, Header(headers::ETag(headers::EntityTag::strong(etag))), file, mt)));
                    }
                    None => false,
                }
            };
            if forgor {
                self.cache_fs_files.write().expect("Filesystem file cache write lock poisoned").retain(|_, v| *v == hash);
                self.cache_fs.write().expect("Filesystem cache write lock poisoned").remove(&cache_key);
                return self.handle_get_file_encoded(req, req_p, mt, headers, etag);
            }

            let mut resp_p = self.encoded_temp_dir.as_ref().unwrap().1.join(cache_key.0.to_hex().as_str());
            match (req_p.extension(), encoding_extension(&encoding)) {
                (Some(ext), Some(enc)) => {
                    let mut new_ext = ext.as_encoded_bytes().to_vec();
                    new_ext.push(b'.');
                    new_ext.extend_from_slice(enc.as_bytes());
                    resp_p.set_extension(unsafe { OsStr::from_encoded_bytes_unchecked(&new_ext) })
                }
                (None, Some(enc)) => resp_p.set_extension(enc),
                (_, None) => unsafe { std::hint::unreachable_unchecked() },
            };

            if encode_file(&req_p, &resp_p, &encoding) {
                let resp_p_len = file_length(&resp_p.metadata().expect("Failed to get encoded file metadata"), &resp_p);
                let gain = (file_length(&req_p.metadata().expect("Failed to get requested file metadata"), &req_p) as f64) / (resp_p_len as f64);
                if gain < MIN_ENCODING_GAIN || resp_p_len > self.encoded_filesystem_limit {
                    let mut cache = self.cache_fs.write().expect("Filesystem cache write lock poisoned");
                    cache.insert(cache_key, ((PathBuf::new(), false, 0), AtomicU64::new(u64::MAX)));
                    fs::remove_file(resp_p).expect("Failed to remove too big encoded file");
                } else {
                    log!(self.log,
                         "{} encoded as {} for {:.1}% ratio",
                         self.remote_addresses(req).as_spaces(),
                         encoding,
                         gain * 100f64);

                    let mut cache = self.cache_fs.write().expect("Filesystem cache write lock poisoned");
                    self.cache_fs_size.fetch_add(resp_p_len, AtomicOrdering::Relaxed);
                    cache.insert(cache_key, ((resp_p.clone(), true, resp_p_len), AtomicU64::new(precise_time_ns())));

                    return Ok(Response::with((status::Ok,
                                              headers,
                                              Header(headers::ETag(headers::EntityTag::strong(etag))),
                                              Header(headers::ContentEncoding([encoding].into())),
                                              resp_p.as_path(),
                                              mt)));
                }
            } else {
                log!(self.log,
                     "{} failed to encode as {}, sending identity",
                     self.remote_addresses(req).as_spaces(),
                     encoding);
            }
        }

        let file = match File::open(&req_p) {
            Ok(file) => file,
            Err(err) => return self.handle_requested_entity_unopenable(req, err, "file"),
        };
        Ok(Response::with((status::Ok,
                           headers,
                           Header(headers::ETag(headers::EntityTag::strong(etag))),
                           Header(headers::ContentLength(file_length(&file.metadata().expect("Failed to get requested file metadata"), &req_p))),
                           file,
                           mt)))
    }

    fn handle_get_raw_fs_dir(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        log!(self.log,
             "{} was served metadata for directory {magenta}{}{reset}",
             self.remote_addresses(&req),
             req_p.display());
        self.handle_raw_fs_api_response(status::Ok,
                                        &FilesetData {
                                            writes_supported: self.writes_temp_dir.is_some(),
                                            is_root: req.url.as_ref().path_segments().unwrap().count() + !req.url.as_ref().as_str().ends_with('/') as usize ==
                                                     1,
                                            is_file: false,
                                            files: req_p.read_dir()
                                                .expect("Failed to read requested directory")
                                                .map(|p| p.expect("Failed to iterate over requested directory"))
                                                .filter(|f| {
                    let fp = f.path();
                    let mut symlink = false;
                    !((!self.follow_symlinks &&
                       {
                        symlink = is_symlink(&fp);
                        symlink
                    }) || (self.follow_symlinks && self.sandbox_symlinks && symlink && !is_descendant_of(fp, &self.hosted_directory.1)))
                })
                                                .map(|f| {
                    let is_file = is_actually_file(&f.file_type().expect("Failed to get file type"), &f.path());
                    if is_file {
                        get_raw_fs_metadata(f.path())
                    } else {
                        RawFileData {
                            mime_type: Mime(MimeTopLevel::Text, MimeSubLevel::Ext("directory".to_string()), Default::default()), // text/directory
                            name: f.file_name().into_string().expect("Failed to get file name"),
                            last_modified: file_time_modified_p(&f.path()).into(),
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
                if req.url.as_ref().path_segments().unwrap().next_back() == Some("") {
                    let r = self.handle_get_file(req, &idx, false);
                    log!(self.log,
                         "{} found index file for directory {magenta}{}{reset}",
                         self.remote_addresses(req).as_spaces(),
                         req_p.display());
                    return r;
                } else {
                    return self.handle_get_dir_index_no_slash(req, e);
                }
            }
        }

        if !self.generate_listings {
            return self.handle_nonexistent_get(req, req_p);
        }

        if client_mobile(&req.headers) {
            self.handle_get_mobile_dir_listing(req, req_p)
        } else {
            self.handle_get_dir_listing(req, req_p)
        }
    }

    fn slashise(u: String) -> String {
        let mut b = u.into_bytes();
        b.insert(b.iter().position(|&c| c == b'?').unwrap_or(b.len()), b'/');
        unsafe { String::from_utf8_unchecked(b) }
    }

    /// Try to resolve any X-Original-URL headers for a redirect, else raw `/loca/tion` from request
    fn user_facing_request_url(&self, req: &Request) -> String {
        for (network, header) in &self.proxy_redirs {
            if network.contains(&req.remote_addr.ip()) {
                if let Some(saddrs) = req.headers.get_raw(header) {
                    if saddrs.len() > 0 {
                        if let Ok(s) = str::from_utf8(&saddrs[0]) {
                            return s.to_string();
                        }
                    }
                }
            }
        }

        req.url.to_string()
    }

    fn handle_get_dir_index_no_slash(&self, req: &mut Request, idx_ext: &str) -> IronResult<Response> {
        let new_url = HttpHandler::slashise(self.user_facing_request_url(req));
        log!(self.log,
             "Redirecting {} to {yellow}{}{reset} - found index file {magenta}index.{}{reset}",
             self.remote_addresses(&req),
             new_url,
             idx_ext);

        // We redirect here because if we don't and serve the index right away funky shit happens.
        // Example:
        //   - Without following slash:
        //     https://cloud.githubusercontent.com/assets/6709544/21442017/9eb20d64-c89b-11e6-8c7b-888b5f70a403.png
        //   - With following slash:
        //     https://cloud.githubusercontent.com/assets/6709544/21442028/a50918c4-c89b-11e6-8936-c29896947f6a.png
        Ok(Response::with((status::SeeOther, Header(headers::Server(USER_AGENT.into())), Header(headers::Location(new_url)))))
    }

    fn handle_get_mobile_dir_listing(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let relpath = url_path(&req.url);
        let is_root = relpath == "/";
        let mut relpath_escaped = escape_specials(&relpath);
        if relpath_escaped.as_bytes().last() != Some(&b'/') {
            relpath_escaped.to_mut().push('/');
        }
        let show_file_management_controls = self.writes_temp_dir.is_some();
        log!(self.log,
             "{} was served mobile directory listing for {magenta}{}{reset}",
             self.remote_addresses(&req),
             req_p.display());

        let parent_f = |out: &mut Vec<u8>| if !is_root {
            let mut parentpath = relpath_escaped.as_bytes();
            while parentpath.last() == Some(&b'/') {
                parentpath = &parentpath[0..parentpath.len() - 1];
            }
            while parentpath.last() != Some(&b'/') {
                parentpath = &parentpath[0..parentpath.len() - 1];
            }
            let modified = file_time_modified_p(req_p.parent().unwrap_or(&req_p));
            let _ = write!(out,
                       r#"<a href="{up_path}" id=".."><div><span class="back_arrow_icon">Parent directory</span></div><div><time ms={}{:03}>{} UTC</time></div></a>"#,
                       modified.timestamp(),
                       modified.timestamp_millis(),
                       modified.format("%F %T"),
                       up_path = unsafe { str::from_utf8_unchecked(parentpath) });
        };
        let list_f = |out: &mut Vec<u8>| {
            let mut list = req_p.read_dir()
                .expect("Failed to read requested directory")
                .map(|p| p.expect("Failed to iterate over requested directory"))
                .filter(|f| {
                    let fp = f.path();
                    let mut symlink = false;
                    !((!self.follow_symlinks &&
                       {
                        symlink = is_symlink(&fp);
                        symlink
                    }) || (self.follow_symlinks && self.sandbox_symlinks && symlink && !is_descendant_of(fp, &self.hosted_directory.1)))
                })
                .collect::<Vec<_>>();
            list.sort_by(|lhs, rhs| {
                (is_actually_file(&lhs.file_type().expect("Failed to get file type"), &lhs.path()),
                 lhs.file_name().to_str().expect("Failed to get file name").to_lowercase())
                    .cmp(&(is_actually_file(&rhs.file_type().expect("Failed to get file type"), &rhs.path()),
                           rhs.file_name().to_str().expect("Failed to get file name").to_lowercase()))
            });
            for f in list {
                let is_file = is_actually_file(&f.file_type().expect("Failed to get file type"), &f.path());
                let fmeta = f.metadata().expect("Failed to get requested file metadata");
                let fname = f.file_name().into_string().expect("Failed to get file name");
                let path = f.path();
                let modified = file_time_modified(&fmeta);

                let _ = writeln!(out,
                                 concat!(r#"<a href="{path}{fname}" id="{}"><div><span class="{}{}_icon">{}{}</span>{}</div>"#,
                                         r#"<div><time ms={}{:03}>{} UTC</time>{}</div></a>"#),
                                 NoDoubleQuotes(&fname),
                                 if is_file { "file" } else { "dir" },
                                 file_icon_suffix(&path, is_file),
                                 NoHtmlLiteral(&fname),
                                 if is_file { "" } else { "/" },
                                 if show_file_management_controls {
                                     DisplayThree(r#"<span class="manage"><span class="delete_file_icon" onclick="delete_onclick(arguments[0])">Delete</span>"#,
                                                  if self.webdav >= WebDavLevel::MkColMoveOnly {
                                                      r#" <span class="rename_icon" onclick="rename_onclick(arguments[0])">Rename</span>"#
                                                  } else {
                                                      ""
                                                  },
                                                  "</span>")
                                 } else {
                                     DisplayThree("", "", "")
                                 },
                                 modified.timestamp(),
                                 modified.timestamp_millis(),
                                 modified.format("%F %T"),
                                 if is_file {
                                     DisplayThree("<span class=\"size\">", Maybe(Some(HumanReadableSize(file_length(&fmeta, &path)))), "</span>")
                                 } else {
                                     DisplayThree("", Maybe(None), "")
                                 },
                                 path = relpath_escaped,
                                 fname = encode_tail_if_trimmed(escape_specials(&fname)));
            }
        };

        self.handle_generated_response_encoding(req,
                                                status::Ok,
                                                directory_listing_mobile_html(&relpath_escaped[!is_root as usize..],
                                                                              if show_file_management_controls {
                                                                                  concat!(r#"<style>"#, include_str!(concat!(env!("OUT_DIR"), "/assets/upload.css")), r#"</style>"#,
                                                                                          r#"<script>"#, include_str!(concat!(env!("OUT_DIR"), "/assets/upload.js")))
                                                                              } else {
                                                                                  ""
                                                                              },
                                                                              if show_file_management_controls {
                                                                                  include_str!(concat!(env!("OUT_DIR"), "/assets/manage_mobile.js"))
                                                                              } else {
                                                                                  ""
                                                                              },
                                                                              if show_file_management_controls {
                                                                                  concat!(include_str!(concat!(env!("OUT_DIR"), "/assets/manage.js")), r#"</script>"#)
                                                                              } else {
                                                                                  ""
                                                                              },
                                                                              parent_f,
                                                                              list_f,
                                                                              if show_file_management_controls {
                                                                                  concat!(r#"<span class="heading">Upload files: "#,
                                                                                          r#"<input type="file" multiple /></span>"#)
                                                                              } else {
                                                                                  ""
                                                                              },
                                                                              if show_file_management_controls && self.webdav >= WebDavLevel::MkColMoveOnly {
                                                                                  r#"<a id='new"directory' href><span class="new_dir_icon">Create directory</span></a>"#
                                                                              } else {
                                                                                  ""
                                                                              }))
    }

    fn handle_get_dir_listing(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let relpath = url_path(&req.url);
        let is_root = relpath == "/";
        let mut relpath_escaped = escape_specials(&relpath);
        if relpath_escaped.as_bytes().last() != Some(&b'/') {
            relpath_escaped.to_mut().push('/');
        }
        let show_file_management_controls = self.writes_temp_dir.is_some();
        log!(self.log,
             "{} was served directory listing for {magenta}{}{reset}",
             self.remote_addresses(&req),
             req_p.display());

        let parent_f = |out: &mut Vec<u8>| if !is_root {
            let mut parentpath = relpath_escaped.as_bytes();
            while parentpath.last() == Some(&b'/') {
                parentpath = &parentpath[0..parentpath.len() - 1];
            }
            while parentpath.last() != Some(&b'/') {
                parentpath = &parentpath[0..parentpath.len() - 1];
            }
            let modified = file_time_modified_p(req_p.parent().unwrap_or(&req_p));
            let _ = write!(out,
                           "<tr id=\"..\"><td><a href=\"{up_path}\" tabindex=\"-1\" class=\"back_arrow_icon\"></a></td> <td><a \
                            href=\"{up_path}\">Parent directory</a></td> <td><a href=\"{up_path}\" tabindex=\"-1\"><time ms={}{:03}>{}</time></a></td> \
                            <td><a href=\"{up_path}\" tabindex=\"-1\">&nbsp;</a></td> <td><a href=\"{up_path}\" tabindex=\"-1\">&nbsp;</a></td></tr>",
                           modified.timestamp(),
                           modified.timestamp_millis(),
                           modified.format("%F %T"),
                           up_path = unsafe { str::from_utf8_unchecked(parentpath) });
        };

        let rd = match req_p.read_dir() {
            Ok(rd) => rd,
            Err(err) => return self.handle_requested_entity_unopenable(req, err, "directory"),
        };
        let list_f = |out: &mut Vec<u8>| {
            let mut list = rd.map(|p| p.expect("Failed to iterate over requested directory"))
                .filter(|f| {
                    let fp = f.path();
                    let mut symlink = false;
                    !((!self.follow_symlinks &&
                       {
                        symlink = is_symlink(&fp);
                        symlink
                    }) || (self.follow_symlinks && self.sandbox_symlinks && symlink && !is_descendant_of(fp, &self.hosted_directory.1)))
                })
                .collect::<Vec<_>>();
            list.sort_by(|lhs, rhs| {
                (is_actually_file(&lhs.file_type().expect("Failed to get file type"), &lhs.path()),
                 lhs.file_name().to_str().expect("Failed to get file name").to_lowercase())
                    .cmp(&(is_actually_file(&rhs.file_type().expect("Failed to get file type"), &rhs.path()),
                           rhs.file_name().to_str().expect("Failed to get file name").to_lowercase()))
            });
            for f in list {
                let path = f.path();
                let is_file = is_actually_file(&f.file_type().expect("Failed to get file type"), &path);
                let fmeta = f.metadata().expect("Failed to get requested file metadata");
                let fname = f.file_name().into_string().expect("Failed to get file name");
                let len = file_length(&fmeta, &path);
                let modified = file_time_modified(&fmeta);
                struct FileSizeDisplay(bool, u64);
                impl fmt::Display for FileSizeDisplay {
                    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        if self.0 {
                            write!(f, "<abbr title=\"{} B\">", self.1)
                        } else {
                            f.write_str("&nbsp;")
                        }
                    }
                }

                let _ = write!(out,
                               "<tr id=\"{}\"><td><a href=\"{path}{fname}\" tabindex=\"-1\" class=\"{}{}_icon\"></a></td> <td><a \
                                href=\"{path}{fname}\">{}{}</a></td> <td><a href=\"{path}{fname}\" tabindex=\"-1\"><time ms={}{:03}>{}</time></a></td> \
                                <td><a href=\"{path}{fname}\" tabindex=\"-1\">{}{}{}</a></td> {}</tr>\n",
                               NoDoubleQuotes(&fname),
                               if is_file { "file" } else { "dir" },
                               file_icon_suffix(&path, is_file),
                               NoHtmlLiteral(&fname),
                               if is_file { "" } else { "/" },
                               modified.timestamp(),
                               modified.timestamp_millis(),
                               modified.format("%F %T"),
                               FileSizeDisplay(is_file, len),
                               if is_file {
                                   Maybe(Some(HumanReadableSize(len)))
                               } else {
                                   Maybe(None)
                               },
                               if is_file { "</abbr>" } else { "" },
                               if show_file_management_controls {
                                   DisplayThree("<td><a href class=\"delete_file_icon\" onclick=\"delete_onclick(arguments[0])\">Delete</a>",
                                                if self.webdav >= WebDavLevel::MkColMoveOnly {
                                                    " <a href class=\"rename_icon\" onclick=\"rename_onclick(arguments[0])\">Rename</a>"
                                                } else {
                                                    ""
                                                },
                                                "</td>")
                               } else {
                                   DisplayThree("", "", "")
                               },
                               path = relpath_escaped,
                               fname = encode_tail_if_trimmed(escape_specials(&fname)));
            }
        };

        self.handle_generated_response_encoding(req,
                                                status::Ok,
                                                directory_listing_html(&relpath_escaped[!is_root as usize..],
                                                                       if show_file_management_controls {
                                                                           concat!(r#"<style>"#,
                                                                                   include_str!(concat!(env!("OUT_DIR"), "/assets/upload.css")),
                                                                                   r#"</style>"#,
                                                                                   r#"<script>"#,
                                                                                   include_str!(concat!(env!("OUT_DIR"), "/assets/upload.js")))
                                                                       } else {
                                                                           ""
                                                                       },
                                                                       if show_file_management_controls {
                                                                           include_str!(concat!(env!("OUT_DIR"), "/assets/manage_desktop.js"))
                                                                       } else {
                                                                           ""
                                                                       },
                                                                       if show_file_management_controls {
                                                                           concat!(include_str!(concat!(env!("OUT_DIR"), "/assets/manage.js")), r#"</script>"#)
                                                                       } else {
                                                                           ""
                                                                       },
                                                                       parent_f,
                                                                       list_f,
                                                                       if show_file_management_controls {
                                                                           "<hr />\
                                                                            <p>Drag&amp;Drop to upload or <input type=\"file\" multiple />.</p>"
                                                                       } else {
                                                                           ""
                                                                       },
                                                                       if show_file_management_controls {
                                                                           "<th>Manage</th>"
                                                                       } else {
                                                                           ""
                                                                       },
                                                                       if show_file_management_controls && self.webdav >= WebDavLevel::MkColMoveOnly {
                                                                           "<tr id=\'new\"directory\'><td><a tabindex=\"-1\" href \
                                                                            class=\"new_dir_icon\"></a></td><td colspan=3><a href>Create \
                                                                            directory</a></td><td><a tabindex=\"-1\" href>&nbsp;</a></td></tr>"
                                                                       } else {
                                                                           ""
                                                                       }))
    }

    fn handle_put(&self, req: &mut Request) -> IronResult<Response> {
        if self.writes_temp_dir.is_none() {
            return self.handle_forbidden_method(req, "-w", "write requests");
        }

        let (req_p, symlink, url_err) = self.parse_requested_path(req);

        if url_err {
            self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>")
        } else if req_p.is_dir() {
            self.handle_disallowed_method(req, "directory")
        } else if detect_file_as_dir(&req_p) {
            self.handle_invalid_url(req, "<p>Attempted to use file as directory.</p>")
        } else if req.headers.has::<headers::ContentRange>() {
            self.handle_put_partial_content(req)
        } else {
            let illegal = (symlink && !self.follow_symlinks) ||
                          (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_nonexistent_descendant_of(&req_p, &self.hosted_directory.1));
            if illegal {
                return self.handle_nonexistent(req, req_p);
            }
            self.handle_put_file(req, req_p)
        }
    }

    fn handle_disallowed_method(&self, req: &mut Request, tpe: &str) -> IronResult<Response> {
        log!(self.log,
             "{} tried to {red}{}{reset} on {magenta}{}{reset} ({blue}{}{reset}) but only {red}{}{reset} are allowed",
             self.remote_addresses(&req),
             req.method,
             url_path(&req.url),
             tpe,
             CommaList(self.allowed_methods.iter()));

        let resp_text = error_html("405 Method Not Allowed",
                                   format_args!("Can't {} on a {}.", req.method, tpe),
                                   format_args!("<p>Allowed methods: {}</p>", CommaList(self.allowed_methods.iter())));
        self.handle_generated_response_encoding(req, status::MethodNotAllowed, resp_text)
            .map(|mut r| {
                r.headers.set(headers::Allow(self.allowed_methods.into()));
                r
            })
    }

    fn handle_put_partial_content(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log,
             "{} tried to {red}PUT{reset} partial content to {yellow}{}{reset}",
             self.remote_addresses(&req),
             url_path(&req.url));

        self.handle_generated_response_encoding(req,
                                                status::BadRequest,
                                                error_html("400 Bad Request",
                                                           "<a href=\"https://tools.ietf.org/html/rfc7231#section-4.3.3\">RFC7231 forbids partial-content \
                                                            PUT requests.</a>",
                                                           ""))
    }

    fn handle_put_file(&self, req: &mut Request, req_p: PathBuf) -> IronResult<Response> {
        let _ = fs::create_dir_all(req_p.parent().expect("Failed to get requested file's parent directory"));
        let direct_output = File::create_new(&req_p);

        let existent = direct_output.is_err();
        let mtime = req.headers.get::<XLastModified>().map(|xlm| xlm.0).or_else(|| req.headers.get::<XOcMTime>().map(|xocmt| xocmt.0 * 1000));
        log!(self.log,
             "{} {} {magenta}{}{reset}, size: {}B{}{}",
             self.remote_addresses(&req),
             if existent { "replaced" } else { "created" },
             req_p.display(),
             *req.headers.get::<headers::ContentLength>().expect("No Content-Length header"),
             mtime.map_or("", |_| ". modified: "),
             Maybe(mtime.map(MsAsS)));

        let mut ibuf = BufReader::with_capacity(1024 * 1024, &mut req.body);
        let file = match direct_output {
            Ok(mut file) => {
                if let Err(err) = io::copy(&mut ibuf, &mut file) {
                    drop(file);
                    fs::remove_file(&req_p).expect("Failed to remove requested file after failure");
                    let _ = io::copy(&mut ibuf, &mut io::sink());
                    return self.handle_put_error(req, "File not created.", err);
                }

                file
            }
            Err(_) => {
                self.create_temp_dir(&self.writes_temp_dir);
                let &(_, ref temp_dir) = self.writes_temp_dir.as_ref().unwrap();
                let temp_file_p = temp_dir.join(req_p.file_name().expect("Failed to get requested file's filename"));
                struct DropDelete<'a>(&'a Path);
                impl<'a> Drop for DropDelete<'a> {
                    fn drop(&mut self) {
                        let _ = fs::remove_file(self.0);
                    }
                }

                let mut temp_file = File::options().read(true).write(true).create(true).truncate(true).open(&temp_file_p).expect("Failed to create temp file");
                let _temp_file_p_destroyer = DropDelete(&temp_file_p);
                if let Err(err) = io::copy(&mut ibuf, &mut temp_file) {
                    let _ = io::copy(&mut ibuf, &mut io::sink());
                    return self.handle_put_error(req, "File not created.", err);
                }

                let _temp_file_p_destroyer = DropDelete(&temp_file_p);
                temp_file.rewind().expect("Failed to rewind temp file");
                let mut file = File::create(&req_p).expect("Failed to open requested file");
                // matches std::io::copy() #[cfg]
                #[cfg(any(target_os = "linux", target_os = "android"))]
                let err = io::copy(&mut temp_file, &mut file);
                #[cfg(not(any(target_os = "linux", target_os = "android")))]
                let err = io::copy(&mut BufReader::with_capacity(1024 * 1024, &mut temp_file), &mut file);
                if let Err(err) = err {
                    return self.handle_put_error(req, "File truncated.", err);
                }

                file
            }
        };

        if let Some(ms) = mtime {
            set_mtime_f(&file, ms);
        }

        Ok(Response::with((if existent {
                               status::NoContent
                           } else {
                               status::Created
                           },
                           Header(headers::Server(USER_AGENT.into())))))
    }

    fn handle_put_error(&self, req: &mut Request, res: &str, err: IoError) -> IronResult<Response> {
        log!(self.log, "{} {} {}", self.remote_addresses(req).as_spaces(), res, err);
        return self.handle_generated_response_encoding(req,
                                                       status::ServiceUnavailable,
                                                       error_html("503 Service Unavailable", res, format_args!("{}", err)));
    }

    fn handle_delete(&self, req: &mut Request) -> IronResult<Response> {
        if self.writes_temp_dir.is_none() {
            return self.handle_forbidden_method(req, "-w", "write requests");
        }

        let (req_p, symlink, url_err) = self.parse_requested_path_custom_symlink(req.url.as_ref(), false);

        if url_err {
            self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>")
        } else if !req_p.exists() || (symlink && !self.follow_symlinks) ||
                  (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&req_p, &self.hosted_directory.1)) {
            self.handle_nonexistent(req, req_p)
        } else {
            self.handle_delete_path(req, req_p, symlink)
        }
    }

    fn handle_delete_path(&self, req: &mut Request, req_p: PathBuf, symlink: bool) -> IronResult<Response> {
        let ft = req_p.metadata().expect("failed to get file metadata").file_type();
        let is_file = is_actually_file(&ft, &req_p);
        log!(self.log,
             "{} deleted {blue}{} {magenta}{}{reset}",
             self.remote_addresses(&req),
             if is_file {
                 "file"
             } else if symlink {
                 "symlink"
             } else {
                 "directory"
             },
             req_p.display());

        if is_file {
            fs::remove_file(req_p).expect("Failed to remove requested file");
        } else {
            fs::remove_dir_all(req_p).expect(if symlink {
                "Failed to remove requested symlink"
            } else {
                "Failed to remove requested directory"
            });
        }

        Ok(Response::with((status::NoContent, Header(headers::Server(USER_AGENT.into())))))
    }

    fn handle_trace(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log,
             "{} requested {red}TRACE{reset} for {magenta}{}{reset}",
             self.remote_addresses(&req),
             url_path(&req.url));

        let mut hdr = mem::replace(&mut req.headers, Headers::new());
        hdr.set(headers::ContentType(Mime(MimeTopLevel::Message, MimeSubLevel::Ext("http".to_string()), Default::default()))); // message/http

        Ok(Response {
            status: Some(status::Ok),
            headers: hdr,
            body: None,
        })
    }

    fn handle_forbidden_method(&self, req: &mut Request, switch: &str, desc: &str) -> IronResult<Response> {
        log!(self.log,
             "{} used disabled request method {red}{}{reset} grouped under {}",
             self.remote_addresses(&req),
             req.method,
             desc);

        self.handle_generated_response_encoding(req,
                                                status::Forbidden,
                                                error_html("403 Forbidden",
                                                           "This feature is currently disabled.",
                                                           format_args!("<p>Ask the server administrator to pass <samp>{}</samp> to the executable to \
                                                                         enable support for {}.</p>",
                                                                        switch,
                                                                        desc)))
    }

    fn handle_bad_method(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log,
             "{} used invalid request method {red}{}{reset}",
             self.remote_addresses(&req),
             req.method);

        self.handle_generated_response_encoding(req,
                                                status::NotImplemented,
                                                error_html("501 Not Implemented",
                                                           "This operation was not implemented.",
                                                           format_args!("<p>Unsupported request method: {}.<br />\nSupported methods: {}.</p>",
                                                                        req.method,
                                                                        CommaList(self.allowed_methods.iter()))))
    }

    fn handle_generated_response_encoding(&self, req: &mut Request, st: status::Status, resp: String) -> IronResult<Response> {
        let hash = blake3::hash(resp.as_bytes());
        let etag = hash.to_string();

        if st == status::Ok && (req.method == method::Get || req.method == method::Head) {
            if let Some(headers::IfNoneMatch::Items(inm)) = req.headers.get::<headers::IfNoneMatch>() {
                if HttpHandler::etag_match(inm, &etag) {
                    log!(self.log, "{} Not Modified", self.remote_addresses(req).as_spaces());
                    return Ok(Response::with((status::NotModified,
                                              Header(headers::Server(USER_AGENT.into())),
                                              Header(headers::ETag(headers::EntityTag::strong(etag))),
                                              text_html_charset_utf8())));
                }
            }
        }

        if let Some(encoding) = req.headers.get_mut::<headers::AcceptEncoding>().and_then(|es| response_encoding(&mut **es)) {
            let cache_key = (hash, encoding.0);

            {
                if let Some(enc_resp) = self.cache_gen.read().expect("Generated file cache read lock poisoned").get(&cache_key) {
                    enc_resp.1.store(precise_time_ns(), AtomicOrdering::Relaxed);
                    log!(self.log,
                         "{} encoded as {} for {:.1}% ratio (cached)",
                         self.remote_addresses(req).as_spaces(),
                         encoding,
                         ((resp.len() as f64) / (enc_resp.0.len() as f64)) * 100f64);

                    return Ok(Response::with((st,
                                              Header(headers::Server(USER_AGENT.into())),
                                              Header(headers::ContentEncoding([encoding].into())),
                                              Header(headers::ETag(headers::EntityTag::strong(etag))),
                                              text_html_charset_utf8(),
                                              &enc_resp.0[..])));
                }
            }

            if let Some(enc_resp) = encode_str(&resp, &encoding) {
                log!(self.log,
                     "{} encoded as {} for {:.1}% ratio",
                     self.remote_addresses(req).as_spaces(),
                     encoding,
                     ((resp.len() as f64) / (enc_resp.len() as f64)) * 100f64);

                if enc_resp.len() as u64 <= self.encoded_generated_limit {
                    let mut cache = self.cache_gen.write().expect("Generated file cache write lock poisoned");
                    self.cache_gen_size.fetch_add(enc_resp.len() as u64, AtomicOrdering::Relaxed);
                    cache.insert(cache_key.clone(), (enc_resp, AtomicU64::new(precise_time_ns())));

                    return Ok(Response::with((st,
                                              Header(headers::Server(USER_AGENT.into())),
                                              Header(headers::ContentEncoding([encoding].into())),
                                              Header(headers::ETag(headers::EntityTag::strong(etag))),
                                              text_html_charset_utf8(),
                                              &cache[&cache_key].0[..])));
                } else {
                    return Ok(Response::with((st,
                                              Header(headers::Server(USER_AGENT.into())),
                                              Header(headers::ContentEncoding([encoding].into())),
                                              Header(headers::ETag(headers::EntityTag::strong(etag))),
                                              text_html_charset_utf8(),
                                              enc_resp)));
                }
            } else {
                log!(self.log,
                     "{} failed to encode as {}, sending identity",
                     self.remote_addresses(req).as_spaces(),
                     encoding);
            }
        }

        Ok(Response::with((st,
                           Header(headers::Server(USER_AGENT.into())),
                           Header(headers::ETag(headers::EntityTag::strong(etag))),
                           text_html_charset_utf8(),
                           resp)))
    }

    fn handle_requested_entity_unopenable(&self, req: &mut Request, e: IoError, entity_type: &str) -> IronResult<Response> {
        if e.kind() == IoErrorKind::PermissionDenied {
            self.handle_generated_response_encoding(req,
                                                    status::Forbidden,
                                                    error_html("403 Forbidden", format_args!("Can't access {}.", url_path(&req.url)), ""))
        } else {
            // The ops that get here (File::open(), fs::read_dir()) can't return any other errors by the time they're run
            // (and even if it could, there isn't much we can do about them)
            panic!("Failed to read requested {}: {:?}", entity_type, e)
        }
    }

    fn handle_raw_fs_api_response<R: Serialize>(&self, st: status::Status, resp: &R) -> IronResult<Response> {
        Ok(Response::with((st,
                           Header(headers::Server(USER_AGENT.into())),
                           Header(RawFsApiHeader(true)),
                           // application/json; charset=utf-8
                           Mime(MimeTopLevel::Application, MimeSubLevel::Json, vec![(MimeAttr::Charset, MimeAttrValue::Utf8)]),
                           serde_json::to_string(&resp).unwrap())))
    }

    fn parse_requested_path(&self, req: &Request) -> (PathBuf, bool, bool) {
        self.parse_requested_path_custom_symlink(req.url.as_ref(), true)
    }

    fn parse_requested_path_custom_symlink(&self, req_url: &GenericUrl, follow_symlinks: bool) -> (PathBuf, bool, bool) {
        let mut depth_left = MAX_SYMLINKS;
        let (mut cur, sk, err, abs) = req_url.path_segments()
            .unwrap()
            .filter(|p| !p.is_empty())
            .fold((self.hosted_directory.1.clone(), false, false, true),
                  |(mut cur, mut sk, mut err, mut abs), pp| {
                if let Some(pp) = percent_decode(pp) {
                    cur.push(&*pp);
                } else {
                    err = true;
                }
                while let Ok(newlink) = cur.read_link() {
                    sk = true;
                    if follow_symlinks && depth_left != 0 {
                        if newlink.is_absolute() {
                            cur = newlink;
                        } else {
                            abs = false;
                            cur.pop();
                            cur.push(newlink);
                        }
                        depth_left -= 1;
                    } else {
                        break;
                    }
                }
                (cur, sk, err, abs)
            });

        if !abs {
            if let Ok(full) = cur.canonicalize() {
                cur = full;
            }
        }

        (cur, sk, err)
    }

    fn create_temp_dir(&self, td: &Option<(String, PathBuf)>) {
        let &(ref temp_name, ref temp_dir) = td.as_ref().unwrap();
        if !temp_dir.exists() && fs::create_dir_all(&temp_dir).is_ok() {
            log!(self.log, "Created temp dir {magenta}{}{reset}", temp_name);
        }
    }

    #[inline(always)]
    fn remote_addresses<'s, 'r, 'ra, 'rb: 'ra>(&'s self, req: &'r Request<'ra, 'rb>) -> AddressWriter<'r, 's, 'ra, 'rb> {
        AddressWriter {
            request: req,
            proxies: &self.proxies,
            log: self.log,
            just_spaces: false,
        }
    }

    fn guess_mime_type(&self, req_p: &Path) -> Mime {
        // Based on mime_guess::guess_mime_type_opt(); that one does to_str() instead of to_string_lossy()
        let ext = req_p.extension().unwrap_or(OsStr::new(""));

        (self.mime_type_overrides.get(&*ext).cloned())
            .or_else(|| ext.to_str().and_then(get_mime_type_opt))
            .unwrap_or_else(|| if file_binary(req_p) {
                Mime(MimeTopLevel::Application, MimeSubLevel::OctetStream, Default::default()) // "application/octet-stream"
            } else {
                Mime(MimeTopLevel::Text, MimeSubLevel::Plain, Default::default()) // "text/plain"
            })
    }
}

/// text/html; charset=utf-8
fn text_html_charset_utf8() -> Mime {
    Mime(MimeTopLevel::Text, MimeSubLevel::Html, vec![(MimeAttr::Charset, MimeAttrValue::Utf8)])
}


pub struct AddressWriter<'r, 'p, 'ra, 'rb: 'ra> {
    pub request: &'r Request<'ra, 'rb>,
    pub proxies: &'p BTreeMap<IpCidr, String>,
    /// (at all, log_time, log_colour)
    pub log: (bool, bool, bool),
    pub just_spaces: bool,
}

impl<'r, 'p, 'ra, 'rb: 'ra> fmt::Display for AddressWriter<'r, 'p, 'ra, 'rb> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use trivial_colours::{Reset as CReset, Colour as C};

        if self.just_spaces {
            return write!(f, "{:w$}", "", w = self.width());
        }

        if self.log.2 {
            write!(f, "{green}{}{reset}", self.request.remote_addr, green = C::Green, reset = CReset)?;
        } else {
            write!(f, "{}", self.request.remote_addr)?;
        }

        for (network, header) in self.proxies {
            if network.contains(&self.request.remote_addr.ip()) {
                if let Some(saddrs) = self.request.headers.get_raw(header) {
                    for saddr in saddrs {
                        if self.log.1 {
                            write!(f, " for {green}{}{reset}", String::from_utf8_lossy(saddr), green = C::Green, reset = CReset)?;
                        } else {
                            write!(f, " for {}", String::from_utf8_lossy(saddr))?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl<'r, 'p, 'ra, 'rb: 'ra> AddressWriter<'r, 'p, 'ra, 'rb> {
    fn maybe_spaces(mut self, ms: bool) -> Self {
        self.just_spaces = ms;
        self
    }

    fn as_spaces(self) -> Self {
        self.maybe_spaces(true)
    }

    fn width(&self) -> usize {
        // per http://192.168.1.109:8000/target/doc/rust/src/core/net/socket_addr.rs.html#571
        const LONGEST_IPV6_SOCKET_ADDR: &str = "[ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff%4294967296]:65536";
        let mut widthbuf = ArrayString::<{ LONGEST_IPV6_SOCKET_ADDR.len() }>::new();
        write!(&mut widthbuf, "{}", self.request.remote_addr).unwrap();
        let mut len = widthbuf.len();
        for (network, header) in self.proxies {
            if network.contains(&self.request.remote_addr.ip()) {
                if let Some(saddrs) = self.request.headers.get_raw(header) {
                    for saddr in saddrs {
                        len += " for ".len();
                        len += saddr.len();
                    }
                }
            }
        }
        len
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
pub fn try_ports<H: Handler + Copy>(hndlr: H, addr: IpAddr, from: u16, up_to: u16, tls_data: &Option<((String, PathBuf), String)>) -> Result<Listening, Error> {
    for port in from..=up_to {
        let ir = Iron::new(hndlr);
        match if let Some(&((_, ref id), ref pw)) = tls_data.as_ref() {
            ir.https((addr, port),
                     NativeTlsServer::new(id, pw).map_err(|err| Error(format!("Opening TLS certificate: {}", err)))?)
        } else {
            ir.http((addr, port))
        } {
            Ok(server) => return Ok(server),
            Err(iron::error::HttpError::Io(ioe)) if ioe.kind() == IoErrorKind::AddrInUse => { /* next */ }
            Err(error) => return Err(Error(format!("Starting server: {}", error))),
        }
    }

    Err(Error(format!("Starting server: no free ports")))
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
    fn err<M: fmt::Display>(which: bool, op: &'static str, more: M) -> Error {
        Error(format!("{} {}: {}",
                      op,
                      if which {
                          "TLS key generation process"
                      } else {
                          "TLS identity generation process"
                      },
                      more))
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
            "Exiting",
            format_args!("{};\nstdout: ```\n{}```;\nstderr: ```\n{}```", exitc, stdout, stderr))
    }

    let tls_dir = temp_dir.1.join("tls");
    fs::create_dir_all(&tls_dir).map_err(|err| Error(format!("Creating temporary directory: {}", err)))?;

    let mut child =
        Command::new("openssl").args(&["req", "-x509", "-newkey", "rsa:4096", "-nodes", "-keyout", "tls.key", "-out", "tls.crt", "-days", "3650", "-utf8"])
            .current_dir(&tls_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| err(true, "Spawning", error))?;
    child.stdin
        .as_mut()
        .unwrap()
        .write_all(concat!("PL\nhttp\n",
                           env!("CARGO_PKG_VERSION"),
                           "\nthecoshman&nabijaczleweli\nнаб\nhttp/",
                           env!("CARGO_PKG_VERSION"),
                           "\nnabijaczleweli@gmail.com\n")
            .as_bytes())
        .map_err(|error| err(true, "Piping", error))?;
    let es = child.wait().map_err(|error| err(true, "Waiting", error))?;
    if !es.success() {
        return Err(exit_err(true, &mut child, &es));
    }

    let mut child = Command::new("openssl").args(&["pkcs12",
                "-export",
                "-out",
                "tls.p12",
                "-inkey",
                "tls.key",
                "-in",
                "tls.crt",
                "-passin",
                "pass:",
                "-passout",
                if cfg!(target_os = "macos") {
                    "pass:password"
                } else {
                    "pass:"
                }])
        .current_dir(&tls_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| err(false, "Spawning", error))?;
    let es = child.wait().map_err(|error| err(false, "Waiting", error))?;
    if !es.success() {
        return Err(exit_err(false, &mut child, &es));
    }

    Ok(((format!("{}/tls/tls.p12", temp_dir.0), tls_dir.join("tls.p12")),
        if cfg!(target_os = "macos") {
                "password"
            } else {
                ""
            }
            .to_string()))
}

/// Generate random username:password auth credentials.
pub fn generate_auth_data() -> String {
    const USERNAME_SET_LEN: usize = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".len();
    const PASSWORD_SET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789~!@#$%^&*()_+`-=[]{}|;',./<>?";


    let rnd = RandomState::new();
    let username_len = (rnd.hash_one((0, 0)) % (12 - 6) + 6) as usize;
    let password_len = (rnd.hash_one((0, 1)) % (25 - 10) + 10) as usize;

    let mut res = String::with_capacity(username_len + 1 + password_len);
    for b in 0..username_len {
        res.push(PASSWORD_SET[(rnd.hash_one((1, b)) % (USERNAME_SET_LEN as u64)) as usize] as char);
    }
    res.push(':');
    for b in 0..password_len {
        res.push(PASSWORD_SET[(rnd.hash_one((2, b)) % (PASSWORD_SET.len() as u64)) as usize] as char);
    }
    res
}
