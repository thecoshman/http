#![allow(unused_imports)]
#![allow(bare_trait_objects)]

use self::super::super::util::{ContentDisposition, DisplayThree, Maybe, extension_is_blacklisted, is_descendant_of, USER_AGENT, MAX_ENCODING_SIZE, MIN_ENCODING_SIZE};
use iron::{headers, status, method, IronResult, Listening, Response, Headers, Request, Handler};
use std::io::{self, ErrorKind as IoErrorKind, BufWriter, Result as IoResult, Write, Read};
use zip::{CompressionMethod as ZipCompressionMethod, DateTime as ZipDateTime};
use iron::mime::{Mime, SubLevel as MimeSubLevel, TopLevel as MimeTopLevel};
use zip::write::{FullFileOptions as ZipFileOptions, ZipWriter};
#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, MetadataExt};
use std::convert::{TryFrom, TryInto};
use tar::Builder as TarBuilder;
use std::path::{PathBuf, Path};
use iron::response::WriteBody;
use self::super::HttpHandler;
use iron::modifiers::Header;
use chrono::{DateTime, Utc};
use std::time::SystemTime;
use std::{fmt, mem, str};
use walkdir::WalkDir;
use std::fs::File;


#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArchiveType {
    Tar,
    Zip,
}

impl ArchiveType {
    pub fn from_mime(mime: &Mime) -> Option<ArchiveType> {
        if mime.0 != MimeTopLevel::Application {
            return None;
        }
        match mime.1.as_str() {
            "x-tar" | "tar" => Some(ArchiveType::Tar), // application/x-tar | application/tar (the second one is an extension for UX)
            "zip" |
            "x-zip-compressed" => Some(ArchiveType::Zip), // application/zip | application/x-zip-compressed
            _ => None,
        }
    }

    pub fn default_mime(self) -> Mime {
        match self {
            ArchiveType::Tar => Mime(MimeTopLevel::Application, MimeSubLevel::Ext("x-tar".into()), Default::default()), // application/x-tar
            ArchiveType::Zip => Mime(MimeTopLevel::Application, MimeSubLevel::Ext("zip".into()), Default::default()), // application/zip
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            ArchiveType::Tar => "tar",
            ArchiveType::Zip => "zip",
        }
    }
}

impl str::FromStr for ArchiveType {
    type Err = ();

    fn from_str(s: &str) -> Result<ArchiveType, ()> {
        let bytes: [u8; 3] = s.as_bytes().try_into().map_err(|_| ())?;
        match &bytes.map(|b| b.to_ascii_lowercase()) {
            b"tar" => Ok(ArchiveType::Tar),
            b"zip" => Ok(ArchiveType::Zip),
            _ => Err(()),
        }
    }
}

impl fmt::Display for ArchiveType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ArchiveType::Tar => f.write_str("tar"),
            ArchiveType::Zip => f.write_str("ZIP"),
        }
    }
}


fn write_tar_body(res: &mut Write, path: &Path) -> IoResult<()> {
    let mut tar = TarBuilder::new(BufWriter::with_capacity(128 * 1024, res));
    tar.follow_symlinks(false);
    if path.is_dir() {
        tar.append_dir_all("", path)?;
    } else {
        tar.append_path_with_name(path, path.file_name().map(Path::new).unwrap_or(path))?;
    }
    tar.into_inner()?.flush()
}

fn write_zip_body(res: &mut Write, path: &Path, allow_encoding: bool) -> IoResult<()> {
    let mut zip = ZipWriter::new_stream(BufWriter::with_capacity(128 * 1024, res));

    for entry in WalkDir::new(&path).follow_links(false).follow_root_links(false).into_iter().flatten() {
        if entry.depth() == 0 && entry.file_type().is_dir() {
            continue
        }

        let relative_path = if entry.depth() == 0 {
            entry.path().file_name().or_else(|| path.file_name()).map(Path::new).unwrap_or(path)
        } else {
            entry.path().strip_prefix(&path).expect("strip_prefix failed; this is a probably a bug in walkdir")
        };
        let Ok(metadata) = entry.metadata() else { continue };

        let mut options = ZipFileOptions::default().compression_method(ZipCompressionMethod::Stored).large_file(metadata.len() >= 2 * 1024 * 1024 * 1024);
        options = match metadata.modified()
            .ok()
            .and_then(|mtime| mtime.duration_since(SystemTime::UNIX_EPOCH).ok())
            .and_then(|mdur| DateTime::<Utc>::from_timestamp_secs(mdur.as_secs() as i64))
            .and_then(|mdt| ZipDateTime::try_from(mdt.naive_utc()).ok()) {
            Some(zdt) => options.last_modified_time(zdt),
            None => options,
        };
        #[cfg(unix)]
        {
            options = options.unix_permissions(metadata.mode());
        }
        #[cfg(not(unix))]
        {
            options = options.unix_permissions((0o644 | (metadata.is_dir() as u32 * 0o111)) & !(metadata.permissions().readonly() as u32 * 0o444));
        }
        // zip can parse these but not generate them and links to https://libzip.org/specifications/extrafld.txt
        // this is the "Timestamp Extra Field"; we may want to have more fields if this is called for
        #[cfg(unix)] // Win32 metadata.change_time() is always None
        {
            let mut ut = vec![0b111];
            for (i, t) in [metadata.mtime(), metadata.atime(), metadata.ctime()].iter().enumerate() {
                ut.extend(t.to_le_bytes());

                if i == 0 {
                    let _ = options.add_extra_data(0x5455, ut.clone().into_boxed_slice(), true);
                }
            }
            let _ = options.add_extra_data(0x5455, ut.into_boxed_slice(), false);
        }

        match metadata.file_type() {
            e if e.is_symlink() => {
                if let Ok(target) = entry.path().read_link() {
                    zip.add_symlink_from_path(relative_path, target, options)?;
                }
            }
            e if e.is_dir() => zip.add_directory_from_path(relative_path, options)?,
            e if e.is_file() => {
                if let Ok(mut opened) = File::open(entry.path()) { // this should have O_NOFOLLOW but it can't
                    let Ok(opened_metadata) = opened.metadata() else { continue };
                    #[cfg(unix)]
                    if opened_metadata.dev() != metadata.dev() || opened_metadata.ino() != metadata.ino() {
                        continue;
                    }

                    if allow_encoding && opened_metadata.len() > MIN_ENCODING_SIZE && opened_metadata.len() < MAX_ENCODING_SIZE &&
                       relative_path.extension().map(|s| !extension_is_blacklisted(s)).unwrap_or(true) {
                        options = options.compression_method(ZipCompressionMethod::Deflated);
                    }

                    zip.start_file_from_path(relative_path, options)?;
                    io::copy(&mut opened, &mut zip)?;
                }
            }
            _ => (), // ZIPs don't support other file types.
        }
    }

    zip.finish()?.flush()
}
fn write_zip_body_no_encoding(res: &mut Write, path: &Path) -> IoResult<()> {
    write_zip_body(res, path, false)
}
fn write_zip_body_yes_encoding(res: &mut Write, path: &Path) -> IoResult<()> {
    write_zip_body(res, path, true)
}


struct WriteArchiveBody((bool, bool, bool), String, ArchiveType, PathBuf, fn(&mut Write, &Path) -> IoResult<()>);
impl WriteBody for WriteArchiveBody {
    fn write_body(&mut self, res: &mut Write) -> IoResult<()> {
        log!(self.0,
             "{} is  served {} archive for {magenta}{}{reset}",
             self.1,
             self.2,
             self.3.display());
        let ret = self.4(res, &self.3);
        log!(self.0,
             "{} was served {} archive for {magenta}{}{reset}{}",
             self.1,
             self.2,
             self.3.display(),
             Maybe(ret.as_ref().err().map(|e| DisplayThree(" – ", e, ""))));
        Ok(())
    }
}


impl HttpHandler {
    /// <form method=post enctype=text/plain> with sentinels matched in generated indices
    /// to avoid pretending we actually support POSTs by accident
    pub(super) fn parse_post_archive(&self, req: &mut Request) -> Option<(ArchiveType, Mime)> {
        // text/plain
        if req.headers.get() == Some(&headers::ContentType(Mime(MimeTopLevel::Text, MimeSubLevel::Plain, Default::default()))) {
            #[allow(invalid_value)]
            let mut buf: [u8; 64] = unsafe { mem::MaybeUninit::uninit().assume_init() };
            let rd = loop {
                match req.body.read(&mut buf) {
                    Ok(rd) => break rd,
                    Err(err) if err.kind() == IoErrorKind::Interrupted => continue,
                    Err(_) => return None,
                }
            };
            let mut ret: Option<ArchiveType> = None;
            let mut vendor = false;
            let mut really = false;
            for l in str::from_utf8(&buf[..rd]).ok()?.lines() {
                if !vendor && l == "vendor=http" {
                    vendor = true;
                } else if !really && l == "archive=yes-i-really-want-one" {
                    really = true;
                } else if let Some(tp) = l.strip_prefix("type=") {
                    ret = tp.parse().ok();
                }
            }
            if vendor && really {
                return ret.map(|at| (at, at.default_mime()));
            }
        }
        None
    }

    /// If Accept: contains application/([x-]tar|zip|x-zip-compressed), then return one match, regardless of quality
    pub(super) fn parse_get_accept_archive(&self, req: &mut Request) -> Option<(ArchiveType, Mime)> {
        req.headers
            .get_mut::<headers::Accept>()
            .and_then(|accept| mem::take(&mut accept.0).into_iter().map(|q| q.item).find_map(|m| ArchiveType::from_mime(&m).map(|at| (at, m))))
    }

    /// As above or GET X-HTTP-Archive: tar|zip
    pub(super) fn handle_get_archive(&self, req: &mut Request, (archive_type, mime): (ArchiveType, Mime)) -> IronResult<Response> {
        let (req_p, symlink, url_err) = self.parse_requested_path(req);
        if url_err {
            return self.handle_invalid_url(req, "<p>Percent-encoding decoded to invalid UTF-8.</p>");
        }

        if !req_p.exists() || (symlink && !self.follow_symlinks) ||
           (symlink && self.follow_symlinks && self.sandbox_symlinks && !is_descendant_of(&req_p, &self.hosted_directory.1)) {
            return self.handle_nonexistent(req, req_p);
        }

        // Not util::url_path(): keep percent-encoded
        let mut attachment = req.url.as_ref().path().trim_matches('/').to_owned();
        if attachment.is_empty() {
            attachment = "all".to_string();
        }
        attachment += ".";
        attachment += archive_type.suffix();

        Ok(Response::with((status::Ok,
                           Header(headers::Server(USER_AGENT.into())),
                           Header(ContentDisposition::Attachment(attachment)),
                           mime,
                           Box::new(WriteArchiveBody(self.log,
                                                     if self.log.0 {
                                                         self.remote_addresses(req).to_string()
                                                     } else {
                                                         String::new()
                                                     },
                                                     archive_type,
                                                     req_p,
                                                     match archive_type {
                                                         ArchiveType::Tar => write_tar_body,
                                                         ArchiveType::Zip => {
                                                             [write_zip_body_no_encoding, write_zip_body_yes_encoding][self.encoded_temp_dir.is_some() as usize]
                                                         }
                                                     })) as Box<WriteBody>)))
    }
}
