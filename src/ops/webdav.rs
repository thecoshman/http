//! Basic WebDAV handling is based heavily on
//! https://github.com/tylerwhall/hyperdav-server/blob/415f512ac030478593ad389a3267aeed7441d826/src/lib.rs,
//! and extended on
//! https://github.com/miquels/webdav-handler-rs @ 02433c1acfccd848a7de26889f6857cbad559076,
//! adhering to
//! https://tools.ietf.org/html/rfc2518


use self::super::super::util::{BorrowXmlName, Destination, CommaList, Overwrite, Depth, win32_file_attributes, file_time_accessed, file_time_modified,
                               file_time_created, client_microsoft, is_actually_file, is_descendant_of, file_executable, html_response, file_length, copy_dir,
                               WEBDAV_ALLPROP_PROPERTIES_NON_WINDOWS, WEBDAV_ALLPROP_PROPERTIES_WINDOWS, WEBDAV_XML_NAMESPACE_MICROSOFT,
                               WEBDAV_XML_NAMESPACE_APACHE, WEBDAV_PROPNAME_PROPERTIES, WEBDAV_XML_NAMESPACE_DAV, WEBDAV_XML_NAMESPACES, ERROR_HTML};
use std::io::{ErrorKind as IoErrorKind, Result as IoResult, Error as IoError, Write, Read};
use xml::reader::{EventReader as XmlReader, XmlEvent as XmlREvent, Error as XmlRError};
use xml::writer::{EventWriter as XmlWriter, XmlEvent as XmlWEvent, Error as XmlWError};
use xml::{EmitterConfig as XmlEmitterConfig, ParserConfig as XmlParserConfig};
use xml::writer::events::StartElementBuilder as XmlWEventStartElementBuilder;
use xml::common::{TextPosition as XmlTextPosition, XmlVersion, Position};
use xml::name::{OwnedName as OwnedXmlName, Name as XmlName};
use iron::{status, IronResult, Response, Request};
use iron::url::Url as GenericUrl;
use std::path::{PathBuf, Path};
use std::fs::{self, Metadata};
use self::super::HttpHandler;
use itertools::Itertools;
use std::borrow::Borrow;
use iron::mime::Mime;
use std::fmt;


lazy_static! {
    static ref DEFAULT_XML_PARSER_CONFIG: XmlParserConfig = XmlParserConfig { trim_whitespace: true, ..Default::default() };
    static ref DEFAULT_XML_EMITTER_CONFIG: XmlEmitterConfig = XmlEmitterConfig { perform_indent: cfg!(debug_assertions), ..Default::default() };
}


impl HttpHandler {
    pub(super) fn handle_webdav_propfind(&self, req: &mut Request) -> IronResult<Response> {
        let (req_p, symlink, url_err) = self.parse_requested_path(req);

        if url_err {
            return self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>");
        }

        if !req_p.exists() || (symlink && !self.follow_symlinks) ||
           (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&req_p, &self.hosted_directory.1)) {
            return self.handle_nonexistent(req, req_p);
        }


        let depth = req.headers.get::<Depth>().copied().unwrap_or(Depth::Zero);

        let props = match parse_propfind(req) {
            Ok(props) => props,
            Err(e) => {
                match match e {
                    Ok(pe) => Ok(pe),
                    Err(xre) => {
                        if xre.position() == XmlTextPosition::new() && xre.msg().contains("no root element") {
                            Err(PropfindVariant::AllProp)
                        } else {
                            Ok(xre.to_string())
                        }
                    }
                } {
                    Ok(e) => {
                        log!(self.log,
                             "{} tried to {red}PROPFIND{reset} {yellow}{}{reset} with invalid XML",
                             self.remote_addresses(&req),
                             req_p.display());
                        return self.handle_generated_response_encoding(req,
                                                                       status::BadRequest,
                                                                       html_response(ERROR_HTML, &["400 Bad Request", &format!("Invalid XML: {}", e), ""]));
                    }
                    Err(props) => props,
                }
            }
        };

        log!(self.log,
             "{} requested {red}PROPFIND{reset} of {} on {yellow}{}{reset} at depth {}",
             self.remote_addresses(&req),
             props,
             req_p.display(),
             depth);

        let url = req.url.as_ref().as_str().to_string();
        let resp = match props {
            PropfindVariant::AllProp => {
                self.handle_webdav_propfind_write_output(req,
                                                         url,
                                                         &req_p,
                                                         if client_microsoft(&req.headers) {
                                                             WEBDAV_ALLPROP_PROPERTIES_WINDOWS
                                                         } else {
                                                             WEBDAV_ALLPROP_PROPERTIES_NON_WINDOWS
                                                         },
                                                         false,
                                                         depth)
            }
            PropfindVariant::PropName => self.handle_webdav_propfind_write_output(req, url, &req_p, WEBDAV_PROPNAME_PROPERTIES, true, depth),
            PropfindVariant::Props(props) => self.handle_webdav_propfind_write_output(req, url, &req_p, &[&props[..]], false, depth),
        };

        match resp.expect("Couldn't write PROPFIND XML") {
            Ok(xml_resp) => Ok(Response::with((status::MultiStatus, xml_resp, "text/xml;charset=utf-8".parse::<Mime>().unwrap()))),
            Err(resp) => resp,
        }
    }

    /// Adapted from
    /// https://github.com/tylerwhall/hyperdav-server/blob/415f512ac030478593ad389a3267aeed7441d826/src/lib.rs#L459
    fn handle_webdav_propfind_write_output<'n, N: BorrowXmlName<'n>>(&self, req: &mut Request, url: String, path: &Path, props: &[&'n [N]], just_names: bool,
                                                                     depth: Depth)
                                                                     -> Result<Result<Vec<u8>, IronResult<Response>>, XmlWError> {
        let mut out = intialise_xml_output()?;
        out.write(namespaces_for_props("D:multistatus", props.iter().flat_map(|pp| pp.iter())))?;

        let meta = path.metadata().expect("Failed to get requested file metadata");
        self.handle_propfind_path(&mut out, &url, &path, &meta, props, just_names)?;

        if meta.is_dir() {
            if let Some(ir) = self.handle_webdav_propfind_path_recursive(req, &mut out, url, &path, props, just_names, depth)? {
                return Ok(Err(ir));
            }
        }

        out.write(XmlWEvent::end_element())?;

        Ok(Ok(out.into_inner()))
    }

    fn handle_webdav_propfind_path_recursive<'n, W: Write, N: BorrowXmlName<'n>>(&self, req: &mut Request, out: &mut XmlWriter<W>, root_url: String,
                                                                                 root_path: &Path, props: &[&'n [N]], just_names: bool, depth: Depth)
                                                                                 -> Result<Option<IronResult<Response>>, XmlWError> {
        if let Some(next_depth) = depth.lower() {
            for f in root_path.read_dir().expect("Failed to read requested directory").map(|p| p.expect("Failed to iterate over requested directory")) {
                let mut url = root_url.clone();
                if !url.ends_with('/') {
                    url.push('/');
                }
                url.push_str(f.file_name().to_str().expect("Filename not UTF-8"));

                let mut path = f.path();
                let mut symlink = false;
                while let Ok(newlink) = path.read_link() {
                    symlink = true;
                    if newlink.is_absolute() {
                        path = newlink;
                    } else {
                        path.pop();
                        path.push(newlink);
                    }
                }

                if !(!path.exists() || (symlink && !self.follow_symlinks) ||
                     (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&path, &self.hosted_directory.1))) {
                    self.handle_propfind_path(out,
                                              &url,
                                              &path,
                                              &path.metadata().expect("Failed to get requested file metadata"),
                                              props,
                                              just_names)?;
                    self.handle_webdav_propfind_path_recursive(req, out, url, &path, props, just_names, next_depth)?;
                }
            }
        }

        Ok(None)
    }

    /// NB: we don't allow modifying any properties, so we 409 Conflict all of them
    pub(super) fn handle_webdav_proppatch(&self, req: &mut Request) -> IronResult<Response> {
        let (req_p, symlink, url_err) = self.parse_requested_path(req);

        if url_err {
            return self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>");
        }

        if self.writes_temp_dir.is_none() {
            return self.handle_forbidden_method(req, "-w", "write requests");
        }

        if !req_p.exists() || (symlink && !self.follow_symlinks) ||
           (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&req_p, &self.hosted_directory.1)) {
            return self.handle_nonexistent(req, req_p);
        }

        let props = match parse_proppatch(req) {
            Ok(props) => props,
            Err(e) => {
                log!(self.log,
                     "{} tried to {red}PROPPATCH{reset} {yellow}{}{reset} with invalid XML",
                     self.remote_addresses(&req),
                     req_p.display());
                return self.handle_generated_response_encoding(req,
                                                               status::BadRequest,
                                                               html_response(ERROR_HTML, &["400 Bad Request", &format!("Invalid XML: {}", e), ""]));
            }
        };

        log!(self.log,
             "{} requested {red}PROPPATCH{reset} of {} on {yellow}{}{reset}",
             self.remote_addresses(&req),
             CommaList(props.iter().map(|p| &p.0.local_name)),
             req_p.display());

        match write_proppatch_output(&props, req.url.as_ref()).expect("Couldn't write PROPPATCH XML") {
            Ok(xml_resp) => Ok(Response::with((status::MultiStatus, xml_resp, "text/xml;charset=utf-8".parse::<Mime>().unwrap()))),
            Err(resp) => resp,
        }
    }

    pub(super) fn handle_webdav_mkcol(&self, req: &mut Request) -> IronResult<Response> {
        let (req_p, symlink, url_err) = self.parse_requested_path(req);

        log!(self.log,
             "{} requested to {red}MKCOL{reset} at {yellow}{}{reset}",
             self.remote_addresses(&req),
             req_p.display());

        if url_err {
            return self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>");
        }

        if self.writes_temp_dir.is_none() {
            return self.handle_forbidden_method(req, "-w", "write requests");
        }

        if !req_p.parent().map(|pp| pp.exists()).unwrap_or(true) || (symlink && !self.follow_symlinks) ||
           (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&req_p, &self.hosted_directory.1)) {
            return self.handle_nonexistent_status(req, req_p, status::Conflict);
        }

        if req.body.read_exact(&mut [0]).is_ok() {
            return Ok(Response::with(status::UnsupportedMediaType));
        }

        match fs::create_dir(&req_p) {
            Ok(()) => Ok(Response::with(status::Created)),
            Err(e) => {
                match e.kind() {
                    IoErrorKind::NotFound => self.handle_nonexistent_status(req, req_p, status::Conflict),
                    IoErrorKind::AlreadyExists => Ok(Response::with((status::MethodNotAllowed, "File exists"))),
                    _ => Ok(Response::with(status::Forbidden)),
                }
            }
        }
    }

    #[inline(always)]
    pub(crate) fn handle_webdav_copy(&self, req: &mut Request) -> IronResult<Response> {
        self.handle_webdav_copy_move(req, false, None)
    }

    #[inline(always)]
    pub(crate) fn handle_webdav_move(&self, req: &mut Request) -> IronResult<Response> {
        let mut sp = (PathBuf::new(), false);
        let resp = self.handle_webdav_copy_move(req, true, Some(&mut sp))?;

        if resp.status == Some(status::Created) || resp.status == Some(status::NoContent) {
            let (req_p, is_file) = sp;

            let removal = if is_file {
                fs::remove_file(req_p)
            } else {
                fs::remove_dir_all(req_p)
            };
            if removal.is_err() {
                return Ok(Response::with(status::Locked));
            }
        }

        Ok(resp)
    }

    fn handle_webdav_copy_move(&self, req: &mut Request, is_move: bool, source_path: Option<&mut (PathBuf, bool)>) -> IronResult<Response> {
        let (req_p, symlink, url_err) = self.parse_requested_path(req);

        if url_err {
            return self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>");
        }
        let (dest_p, dest_symlink) = match req.headers.get::<Destination>() {
            Some(dest) => {
                let (dest_p, dest_symlink, dest_url_err) = self.parse_requested_path_custom_symlink(&dest.0, true);

                if dest_url_err {
                    return self.handle_invalid_url(req, "<p>Percent-encoding decoded destination to invalid UTF-8.</p>");
                }

                (dest_p, dest_symlink)
            }
            None => return self.handle_invalid_url(req, "<p>Destination URL invalid or nonexistent.</p>"),
        };

        let depth = req.headers.get::<Depth>().copied().unwrap_or(Depth::Infinity);
        let overwrite = req.headers.get::<Overwrite>().copied().unwrap_or_default().0;

        log!(self.log,
             "{} requested to {}{red}{}{reset} {yellow}{}{reset} to {yellow}{}{reset} at depth {}",
             self.remote_addresses(&req),
             if overwrite { "overwrite-" } else { "" },
             if !is_move { "COPY" } else { "MOVE" },
             req_p.display(),
             dest_p.display(),
             depth);

        if self.writes_temp_dir.is_none() {
            return self.handle_forbidden_method(req, "-w", "write requests");
        }

        if req_p == dest_p {
            return Ok(Response::with(status::Forbidden));
        }

        if !req_p.exists() || (symlink && !self.follow_symlinks) ||
           (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&req_p, &self.hosted_directory.1)) {
            return self.handle_nonexistent(req, req_p);
        }

        if !dest_p.parent().map(|pp| pp.exists()).unwrap_or(true) || (dest_symlink && !self.follow_symlinks) ||
           (dest_symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&dest_p, &self.hosted_directory.1)) {
            return Ok(Response::with(status::Conflict));
        }

        let mut overwritten = false;
        if dest_p.exists() {
            if !overwrite {
                return Ok(Response::with(status::PreconditionFailed));
            }

            if !is_actually_file(&dest_p.metadata().expect("Failed to get destination file metadata").file_type()) {
                // NB: this disallows overwriting non-empty directories
                if fs::remove_dir(&dest_p).is_err() {
                    return Ok(Response::with(status::Locked));
                }
            }

            overwritten = true;
        }

        let source_file = is_actually_file(&req_p.metadata().expect("Failed to get requested file metadata").file_type());
        if let Some(sp) = source_path {
            *sp = (req_p.clone(), source_file);
        }
        if source_file {
            copy_response(fs::copy(req_p, dest_p).map(|_| ()), overwritten)
        } else {
            match depth {
                Depth::Zero if !is_move => copy_response(fs::create_dir(dest_p), overwritten),
                Depth::Infinity => {
                    match copy_dir(&req_p, &dest_p) {
                        Ok(errors) => {
                            if errors.is_empty() {
                                copy_response(Ok(()), overwritten)
                            } else {
                                Ok(Response::with((status::MultiStatus,
                                                   copy_response_multierror(&errors, req.url.as_ref()).expect("Couldn't write PROPFIND XML"))))
                            }
                        }
                        Err(err) => copy_response(Err(err), overwritten),
                    }
                }
                _ => {
                    self.handle_generated_response_encoding(req,
                                                            status::BadRequest,
                                                            html_response(ERROR_HTML, &["400 Bad Request", &format!("Invalid depth: {}", depth), ""]))
                }
            }
        }
    }

    /// Adapted from
    /// https://github.com/tylerwhall/hyperdav-server/blob/415f512ac030478593ad389a3267aeed7441d826/src/lib.rs#L306
    fn handle_propfind_path<'n, W: Write, N: BorrowXmlName<'n>>(&self, out: &mut XmlWriter<W>, url: &str, path: &Path, meta: &Metadata, props: &[&'n [N]],
                                                                just_names: bool)
                                                                -> Result<(), XmlWError> {
        out.write(XmlWEvent::start_element("D:response"))?;

        out.write(XmlWEvent::start_element("D:href"))?;
        out.write(XmlWEvent::characters(url))?;
        out.write(XmlWEvent::end_element())?; // href

        let prop_count = props.iter().map(|pp| pp.len()).sum();
        let mut failed_props = Vec::with_capacity(prop_count);
        out.write(XmlWEvent::start_element("D:propstat"))?;
        out.write(XmlWEvent::start_element("D:prop"))?;
        for prop in props.iter().flat_map(|pp| pp.iter()) {
            let prop = prop.borrow_xml_name();

            let mut write_name = false;
            if !just_names && !self.handle_prop_path(out, path, meta, prop)? {
                failed_props.push(prop);
                write_name = true;
            }

            if just_names || write_name {
                start_client_prop_element(out, prop)?;
                out.write(XmlWEvent::end_element())?;
            }
        }
        out.write(XmlWEvent::end_element())?; // prop
        out.write(XmlWEvent::start_element("D:status"))?;
        if failed_props.len() >= prop_count {
            // If they all failed, make this a failure response and return
            out.write(XmlWEvent::characters("HTTP/1.1 404 Not Found"))?;
            out.write(XmlWEvent::end_element())?; // status
            out.write(XmlWEvent::end_element())?; // propstat
            out.write(XmlWEvent::end_element())?; // response
            return Ok(());
        }

        out.write(XmlWEvent::characters("HTTP/1.1 200 OK"))?;
        out.write(XmlWEvent::end_element())?; // status
        out.write(XmlWEvent::end_element())?; // propstat

        if !failed_props.is_empty() {
            // Handle the failed properties
            out.write(XmlWEvent::start_element("D:propstat"))?;
            out.write(XmlWEvent::start_element("D:prop"))?;
            for prop in failed_props {
                start_client_prop_element(out, prop)?;
                out.write(XmlWEvent::end_element())?;
            }
            out.write(XmlWEvent::end_element())?; // prop
            out.write(XmlWEvent::start_element("D:status"))?;
            out.write(XmlWEvent::characters("HTTP/1.1 404 Not Found"))?;
            out.write(XmlWEvent::end_element())?; // status
            out.write(XmlWEvent::end_element())?; // propstat
        }

        out.write(XmlWEvent::end_element())?; // response

        Ok(())
    }

    /// Adapted from
    /// https://github.com/tylerwhall/hyperdav-server/blob/415f512ac030478593ad389a3267aeed7441d826/src/lib.rs#L245
    /// extended properties adapted from
    /// https://github.com/miquels/webdav-handler-rs/blob/02433c1acfccd848a7de26889f6857cbad559076/src/handle_props.rs#L655
    fn handle_prop_path<W: Write>(&self, out: &mut XmlWriter<W>, path: &Path, meta: &Metadata, prop: XmlName) -> Result<bool, XmlWError> {
        if prop.namespace == Some(WEBDAV_XML_NAMESPACE_DAV.1) {
            match prop.local_name {
                "creationdate" => {
                    out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_DAV.0, "creationdate")))?;
                    out.write(XmlWEvent::characters(&file_time_created(meta).rfc3339().to_string()))?;
                }

                "getcontentlength" => {
                    out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_DAV.0, "getcontentlength")))?;
                    out.write(XmlWEvent::characters(&file_length(&meta, &path).to_string()))?;
                }

                "getcontenttype" => {
                    out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_DAV.0, "getcontenttype")))?;
                    out.write(XmlWEvent::characters(&self.guess_mime_type(&path).to_string()))?;
                }

                "getlastmodified" => {
                    out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_DAV.0, "getlastmodified")))?;
                    out.write(XmlWEvent::characters(&file_time_modified(meta).rfc3339().to_string()))?;
                }

                "resourcetype" => {
                    out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_DAV.0, "resourcetype")))?;
                    if !is_actually_file(&meta.file_type()) {
                        out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_DAV.0, "collection")))?;
                        out.write(XmlWEvent::end_element())?;
                    }
                }

                _ => return Ok(false),
            }
        } else if prop.namespace == Some(WEBDAV_XML_NAMESPACE_MICROSOFT.1) {
            match prop.local_name {
                "Win32CreationTime" => {
                    out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_MICROSOFT.0, "Win32CreationTime")))?;
                    out.write(XmlWEvent::characters(&file_time_created(meta).rfc3339().to_string()))?;
                }

                "Win32FileAttributes" => {
                    out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_MICROSOFT.0, "Win32FileAttributes")))?;

                    let attr = win32_file_attributes(meta, path);
                    out.write(XmlWEvent::characters(&format!("{:08x}", attr)))?;
                }

                "Win32LastAccessTime" => {
                    out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_MICROSOFT.0, "Win32FileAttributes")))?;
                    out.write(XmlWEvent::characters(&file_time_accessed(meta).rfc3339().to_string()))?;
                }

                "Win32LastModifiedTime" => {
                    out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_MICROSOFT.0, "Win32LastModifiedTime")))?;
                    out.write(XmlWEvent::characters(&file_time_modified(meta).rfc3339().to_string()))?;
                }

                _ => return Ok(false),
            }
        } else if prop.namespace == Some(WEBDAV_XML_NAMESPACE_APACHE.1) {
            match prop.local_name {
                "executable" => {
                    out.write(XmlWEvent::start_element((WEBDAV_XML_NAMESPACE_APACHE.0, "executable")))?;
                    out.write(XmlWEvent::characters(if file_executable(&meta) { "T" } else { "F" }))?;
                }

                _ => return Ok(false),
            }
        } else {
            return Ok(false);
        }

        out.write(XmlWEvent::end_element())?;
        Ok(true)
    }
}


/// https://tools.ietf.org/html/rfc2518#section-12.14
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum PropfindVariant {
    AllProp,
    PropName,
    Props(Vec<OwnedXmlName>),
}

impl fmt::Display for PropfindVariant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PropfindVariant::AllProp => f.write_str("all props"),
            PropfindVariant::PropName => f.write_str("prop names"),
            PropfindVariant::Props(props) => {
                let mut itr = props.iter();
                if let Some(name) = itr.next() {
                    name.borrow().repr_display().fmt(f)?;

                    for name in itr {
                        f.write_str(", ")?;
                        name.borrow().repr_display().fmt(f)?;
                    }
                }

                Ok(())
            }
        }
    }
}


/// https://tools.ietf.org/html/rfc2518#section-12.14
///
/// Adapted from
/// https://github.com/tylerwhall/hyperdav-server/blob/415f512ac030478593ad389a3267aeed7441d826/src/lib.rs#L158
fn parse_propfind(req: &mut Request) -> Result<PropfindVariant, Result<String, XmlRError>> {
    #[derive(Debug, Copy, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
    enum State {
        Start,
        PropFind,
        Prop,
        InProp,
    }


    let mut xml = XmlReader::new_with_config(&mut req.body, DEFAULT_XML_PARSER_CONFIG.clone());
    let mut state = State::Start;
    let mut props = vec![];

    loop {
        let event = xml.next().map_err(Err)?;

        match (state, event) {
            (State::Start, XmlREvent::StartDocument { .. }) => (),
            (State::Start, XmlREvent::StartElement { ref name, .. }) if name.local_name == "propfind" => state = State::PropFind,

            (State::PropFind, XmlREvent::StartElement { ref name, .. }) if name.local_name == "allprop" => return Ok(PropfindVariant::AllProp),
            (State::PropFind, XmlREvent::StartElement { ref name, .. }) if name.local_name == "propname" => return Ok(PropfindVariant::PropName),
            (State::PropFind, XmlREvent::StartElement { ref name, .. }) if name.local_name == "prop" => state = State::Prop,

            (State::Prop, XmlREvent::StartElement { name, .. }) => {
                state = State::InProp;
                props.push(name);
            }
            (State::Prop, XmlREvent::EndElement { .. }) => return Ok(PropfindVariant::Props(props)),

            (State::InProp, XmlREvent::EndElement { .. }) => state = State::Prop,

            (st, ev) => return Err(Ok(format!("Unexpected event {:?} during state {:?}", ev, st))),
        }
    }
}

/// Adapted from
/// https://github.com/tylerwhall/hyperdav-server/blob/415f512ac030478593ad389a3267aeed7441d826/src/lib.rs#L214
fn start_client_prop_element<W: Write>(out: &mut XmlWriter<W>, prop: XmlName) -> Result<(), XmlWError> {
    if let Some(prop_namespace) = prop.namespace {
        if let Some((prefix, _)) = WEBDAV_XML_NAMESPACES.iter().find(|(_, ns)| *ns == prop_namespace) {
            return out.write(XmlWEvent::start_element(XmlName { prefix: Some(prefix), ..prop }));
        }

        if prop.prefix.map(|prop_prefix| WEBDAV_XML_NAMESPACES.iter().any(|(pf, _)| *pf == prop_prefix)).unwrap_or(true) {
            return out.write(XmlWEvent::start_element(XmlName { prefix: Some("U"), ..prop }).ns("U", prop_namespace));
        }
    }

    out.write(XmlWEvent::start_element(prop))
}

/// https://tools.ietf.org/html/rfc2518#section-12.13
fn parse_proppatch(req: &mut Request) -> Result<Vec<(OwnedXmlName, bool)>, String> {
    #[derive(Debug, Copy, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
    enum State {
        Start,
        PropertyUpdate,
        Action,
        Prop,
        InProp,
    }


    let mut xml = XmlReader::new_with_config(&mut req.body, DEFAULT_XML_PARSER_CONFIG.clone());
    let mut state = State::Start;
    let mut props = vec![];
    let mut propname = None;
    let mut is_remove = false;

    loop {
        let event = xml.next().map_err(|e| e.to_string())?;

        match (state, event) {
            (State::Start, XmlREvent::StartDocument { .. }) => (),
            (State::Start, XmlREvent::StartElement { ref name, .. }) if name.local_name == "propertyupdate" => state = State::PropertyUpdate,

            (State::PropertyUpdate, XmlREvent::StartElement { ref name, .. }) if name.local_name == "set" => {
                state = State::Action;
                is_remove = false;
            }
            (State::PropertyUpdate, XmlREvent::StartElement { ref name, .. }) if name.local_name == "remove" => {
                state = State::Action;
                is_remove = true;
            }
            (State::PropertyUpdate, XmlREvent::EndElement { .. }) => return Ok(props),

            (State::Action, XmlREvent::StartElement { ref name, .. }) if name.local_name == "prop" => state = State::Prop,
            (State::Action, XmlREvent::EndElement { .. }) => state = State::PropertyUpdate,

            (State::Prop, XmlREvent::StartElement { name, .. }) => {
                state = State::InProp;
                propname = Some(name.clone());
                props.push((name, is_remove));
            }
            (State::Prop, XmlREvent::EndElement { .. }) => state = State::Action,

            (State::InProp, XmlREvent::EndElement { name, .. }) => {
                if Some(name) == propname {
                    state = State::Prop;
                }
            }
            (State::InProp, _) => {}

            (st, ev) => return Err(format!("Unexpected event {:?} during state {:?}", ev, st)),
        }
    }
}

fn write_proppatch_output(props: &[(OwnedXmlName, bool)], req_url: &GenericUrl) -> Result<Result<Vec<u8>, IronResult<Response>>, XmlWError> {
    let mut out = intialise_xml_output()?;
    out.write(namespaces_for_props("D:multistatus", props.iter().map(|pp| &pp.0)))?;

    out.write(XmlWEvent::start_element("D:href"))?;
    out.write(XmlWEvent::characters(req_url.as_str()))?;
    out.write(XmlWEvent::end_element())?;

    out.write(XmlWEvent::start_element("D:propstat"))?;

    for (name, _) in props {
        out.write(XmlWEvent::start_element("D:prop"))?;

        start_client_prop_element(&mut out, name.borrow())?;
        out.write(XmlWEvent::end_element())?;

        out.write(XmlWEvent::end_element())?;
    }

    out.write(XmlWEvent::start_element("D:status"))?;
    out.write(XmlWEvent::characters("HTTP/1.1 409 Conflict"))?;
    out.write(XmlWEvent::end_element())?;

    out.write(XmlWEvent::end_element())?;

    out.write(XmlWEvent::end_element())?;

    Ok(Ok(out.into_inner()))
}

fn copy_response(op_result: IoResult<()>, overwritten: bool) -> IronResult<Response> {
    match op_result {
        Ok(_) => {
            if overwritten {
                Ok(Response::with(status::NoContent))
            } else {
                Ok(Response::with(status::Created))
            }
        }
        Err(_) => Ok(Response::with(status::InsufficientStorage)),
    }
}

fn copy_response_multierror(errors: &[(IoError, String)], req_url: &GenericUrl) -> Result<Vec<u8>, XmlWError> {
    let mut out = intialise_xml_output()?;
    out.write(XmlWEvent::start_element("D:multistatus").ns(WEBDAV_XML_NAMESPACE_DAV.0, WEBDAV_XML_NAMESPACE_DAV.1))?;
    out.write(XmlWEvent::start_element("D:response"))?;

    for (_, subp) in errors {
        out.write(XmlWEvent::start_element("D:href"))?;
        out.write(XmlWEvent::characters(req_url.join(subp).expect("Couldn't append errored path to url").as_str()))?;
        out.write(XmlWEvent::end_element())?;
    }

    out.write(XmlWEvent::start_element("D:status"))?;
    out.write(XmlWEvent::characters("HTTP/1.1 507 Insufficient Storage"))?;
    out.write(XmlWEvent::end_element())?;

    out.write(XmlWEvent::end_element())?;

    out.write(XmlWEvent::end_element())?;

    Ok(out.into_inner())
}

fn intialise_xml_output() -> Result<XmlWriter<Vec<u8>>, XmlWError> {
    let mut out = XmlWriter::new_with_config(vec![], DEFAULT_XML_EMITTER_CONFIG.clone());

    out.write(XmlWEvent::StartDocument {
            version: XmlVersion::Version10,
            encoding: Some("utf-8"),
            standalone: None,
        })?;

    Ok(out)
}

fn namespaces_for_props<'n, N: 'n + BorrowXmlName<'n>, Ni: Iterator<Item = &'n N>>(elem_name: &str, props: Ni) -> XmlWEventStartElementBuilder {
    let mut bldr = XmlWEvent::start_element(elem_name).ns(WEBDAV_XML_NAMESPACES[0].0, WEBDAV_XML_NAMESPACES[0].1);

    for prop_namespace in props.map(|p| p.borrow().borrow_xml_name()).flat_map(|p| p.namespace).unique() {
        if let Some((prefix, namespace)) = WEBDAV_XML_NAMESPACES[1..].iter().find(|(_, ns)| *ns == prop_namespace) {
            bldr = bldr.ns(*prefix, *namespace);
        }
    }

    bldr
}
