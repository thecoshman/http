//! This module contains the configuration of the application.
//!
//! All options are passed individually to each function and are not bundled together.
//!
//! # Examples
//!
//! ```no_run
//! # use https::Options;
//! let options = Options::parse();
//! println!("Directory to host: {}", options.hosted_directory.0);
//! ```


use clap::{AppSettings, ErrorKind as ClapErrorKind, Error as ClapError, Arg, App};
use std::collections::btree_map::{BTreeMap, Entry as BTreeMapEntry};
use self::super::ops::WebDavLevel;
use std::ffi::{OsString, OsStr};
use std::collections::BTreeSet;
use std::env::{self, temp_dir};
use std::num::NonZeroU64;
use std::{cmp, str, fs};
use std::path::PathBuf;
use std::str::FromStr;
use std::borrow::Cow;
use iron::mime::Mime;
use std::net::IpAddr;
use cidr::IpCidr;
use blake3;


#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Write everything
    All,
    /// No serving messages
    NoServeStatus,
    /// No startup messages, but yes auth data
    NoStartup,
    /// No auth data
    NoAuth,
}

impl From<u64> for LogLevel {
    fn from(raw: u64) -> LogLevel {
        match raw {
            0 => LogLevel::All,
            1 => LogLevel::NoServeStatus,
            2 => LogLevel::NoStartup,
            _ => LogLevel::NoAuth,
        }
    }
}


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// The directory to host.
    pub hosted_directory: (String, PathBuf),
    /// The port to host on. Default: first free port from 8000 up
    pub port: Option<u16>,
    /// The address to bind to. Default: 0.0.0.0
    pub bind_address: IpAddr,
    /// Whether to allow symlinks to be requested. Default: true
    pub follow_symlinks: bool,
    /// Whether to disallow going out of the descendants of the hosted directory (via symlinks)
    ///
    /// Can only be true if `follow_symlinks` is true.
    ///
    /// Default: false
    pub sandbox_symlinks: bool,
    /// The temp directory to write to before copying to hosted directory and to store encoded FS responses.
    /// Default: `"$TEMP/http-[FULL_PATH_TO_HOSTED_DIR]"`
    pub temp_directory: (String, PathBuf),
    /// Whether to generate directory listings at all. Default: true
    pub generate_listings: bool,
    /// Whether to check for index files in served directories before serving a listing. Default: true
    pub check_indices: bool,
    /// Whether to allow requests to `/file` to return `/file.{INDEX_EXTENSIONS`. Default: false
    pub strip_extensions: bool,
    /// Instead of returning 404, try this file first. Default: `None`
    pub try_404: Option<PathBuf>,
    /// Whether to allow write operations. Default: false
    pub allow_writes: bool,
    /// Whether to encode filesystem files. Default: true
    pub encode_fs: bool,
    /// Consume at most this much space for encoded filesystem files.
    pub encoded_filesystem_limit: Option<u64>,
    /// Consume at most this much memory for encoded generated responses.
    pub encoded_generated_limit: Option<u64>,
    /// Prune cached encoded data older than this many seconds.
    pub encoded_prune: Option<u64>,
    /// How much to suppress output
    ///
    ///   * >= 1 – suppress serving status lines ("IP was served something")
    ///   * >= 2 – suppress startup except for auth data, if present
    ///   * >= 3 – suppress all startup messages
    pub loglevel: LogLevel,
    /// Whether to include the time in the log output. Default: `true`
    pub log_time: bool,
    /// Whether to colourise the log output. Default: `true`
    pub log_colour: bool,
    /// Whether to handle WebDAV requests. Default: false
    pub webdav: WebDavLevel,
    /// Data for HTTPS, identity file and password. Default: `None`
    pub tls_data: Option<((String, PathBuf), String)>,
    /// Whether to generate a one-off certificate. Default: false
    pub generate_tls: bool,
    /// Data for per-path authentication, in the form `username[:password]`, or `None` to explicitly disable
    pub path_auth_data: BTreeMap<String, Option<String>>,
    /// Paths for which to generate auth data
    pub generate_path_auth: BTreeSet<String>,
    /// Header names and who we trust them from in `HEADER-NAME:CIDR` format
    pub proxies: BTreeMap<IpCidr, String>,
    /// Header names and who we trust them from in `HEADER-NAME:CIDR` format
    pub proxy_redirs: BTreeMap<IpCidr, String>,
    /// Extension -> MIME type mapping overrides; empty string for no extension
    pub mime_type_overrides: BTreeMap<OsString, Mime>,
    /// Max amount of data per second each request is allowed to return. Default: `None`
    pub request_bandwidth: Option<NonZeroU64>,
    /// Additional headers to add to every response
    pub additional_headers: Vec<(String, Vec<u8>)>,
}

impl Options {
    /// Parse `env`-wide command-line arguments into an `Options` instance
    pub fn parse() -> Options {
        let matches = App::new("http")
            .version(crate_version!())
            .author(&*env!("CARGO_PKG_AUTHORS").replace(":", "\n"))
            .about(crate_description!())
            .setting(AppSettings::ColoredHelp)
            .arg(Arg::from_usage("[DIR] 'Directory to host. Default: current working directory'")
                .validator(|s| Options::filesystem_dir_validator(s, "Directory to host")))
            .arg(Arg::from_usage("-p --port [port] 'Port to use. Default: first free port from 8000 up'").validator(Options::u16_validator))
            .arg(Arg::from_usage("-a --address [address] 'Address to bind to. Default: 0.0.0.0'").validator(Options::ipaddr_validator))
            .arg(Arg::from_usage("-t --temp-dir [temp] 'Temporary directory. Default: $TEMP'")
                .validator(|s| Options::filesystem_dir_validator(s, "Temporary directory")))
            .arg(Arg::from_usage("--404 [fallback-file] 'Return this file instead of a 404 for a GET. Default: generated response'"))
            .arg(Arg::from_usage("-s --no-follow-symlinks 'Don't follow symlinks. Default: false'"))
            .arg(Arg::from_usage("-r --sandbox-symlinks 'Restrict/sandbox where symlinks lead to only the direct descendants of the hosted directory. \
                                  Default: false'"))
            .arg(Arg::from_usage("-w --allow-write 'Allow for write operations. Default: false'"))
            .arg(Arg::from_usage("-l --no-listings 'Never generate dir listings. Default: false'"))
            .arg(Arg::from_usage("-i --no-indices 'Do not automatically use index files. Default: false'"))
            .arg(Arg::from_usage("-e --no-encode 'Do not encode filesystem files. Default: false'"))
            .arg(Arg::from_usage("--encoded-filesystem [FS_LIMIT] 'Consume at most FS_LIMIT space for encoded filesystem files.'")
                .validator(|s| Options::size_parse(s.into()).map(|_| ())))
            .arg(Arg::from_usage("--encoded-generated [GEN_LIMIT] 'Consume at most GEN_LIMIT memory for encoded generated responses.'")
                .validator(|s| Options::size_parse(s.into()).map(|_| ())))
            .arg(Arg::from_usage("--encoded-prune [MAX_AGE] 'Prune cached encoded data older than MAX_AGE.'")
                .validator(|s| Options::age_parse(s.into()).map(|_| ())))
            .arg(Arg::from_usage("-x --strip-extensions 'Allow stripping index extensions from served paths. Default: false'"))
            .arg(Arg::from_usage("-q --quiet... 'Suppress increasing amounts of output'"))
            .arg(Arg::from_usage("-Q --quiet-time 'Don't prefix logs with the timestamp'"))
            .arg(Arg::from_usage("-c --no-colour 'Don't colourise the log output'"))
            .arg(Arg::from_usage("-d --webdav 'Handle WebDAV requests. Default: false'"))
            .arg(Arg::from_usage("-D --convenient-webdav 'Allow WebDAV MKCOL and MOVE only. Default: false'"))
            .arg(Arg::from_usage("--ssl [TLS_IDENTITY] 'Data for HTTPS, identity file. Password in HTTP_SSL_PASS env var, otherwise empty'")
                .validator(Options::identity_validator))
            .arg(Arg::from_usage("--gen-ssl 'Generate a one-off TLS certificate'").conflicts_with("ssl"))
            .arg(Arg::from_usage("--auth [USERNAME[:PASSWORD]] 'Data for global authentication'").validator(Options::credentials_validator))
            .arg(Arg::from_usage("--gen-auth 'Generate a one-off username:password set for global authentication'").conflicts_with("auth"))
            .arg(Arg::from_usage("--path-auth [PATH=[USERNAME[:PASSWORD]]]... 'Data for authentication under PATH'")
                .number_of_values(1)
                .use_delimiter(false)
                .validator(Options::path_credentials_validator))
            .arg(Arg::from_usage("--gen-path-auth [PATH]... 'Generate a one-off username:password set for authentication under PATH'")
                .number_of_values(1)
                .use_delimiter(false))
            .arg(Arg::from_usage("--proxy [HEADER-NAME:CIDR]... 'Treat HEADER-NAME as proxy forwarded-for header when request comes from CIDR'")
                .number_of_values(1)
                .use_delimiter(false)
                .validator(|s| Options::proxy_parse(s.into()).map(|_| ())))
            .arg(Arg::from_usage("--proxy-redir [HEADER-NAME:CIDR]... 'Treat HEADER-NAME as proxy X-Original-URL header for redirects when request comes \
                                  from CIDR'")
                .number_of_values(1)
                .use_delimiter(false)
                .validator(|s| Options::proxy_parse(s.into()).map(|_| ())))
            .arg(Arg::from_usage("-m --mime-type [EXTENSION:MIME-TYPE]... 'Always return MIME-TYPE for files with EXTENSION'")
                .number_of_values(1)
                .use_delimiter(false)
                .validator_os(|s| Options::mime_type_override_parse(s.into()).map(|_| ())))
            .arg(Arg::from_usage("--request-bandwidth [BYTES] 'Limit each request to returning BYTES per second, or 0 for unlimited. Default: 0'")
                .validator(|s| Options::bandwidth_parse(s.into()).map(|_| ())))
            .arg(Arg::from_usage("-H --header [NAME: VALUE]... 'Headers to add to every response'")
                .number_of_values(1)
                .use_delimiter(false)
                .validator(|s| Options::header_parse(&s).map(|_| ())))
            .get_matches();

        let dir = matches.value_of("DIR").unwrap_or(".");
        let dir_pb = fs::canonicalize(dir).unwrap();
        let follow_symlinks = !matches.is_present("no-follow-symlinks");

        let mut path_auth_data = BTreeMap::new();
        if let Some(root_auth) = matches.value_of("auth").map(Options::normalise_credentials) {
            path_auth_data.insert("".to_string(), Some(root_auth));
        }

        if let Some(path_auth) = matches.values_of("path-auth") {
            for (path, auth) in path_auth.map(Options::decode_path_credentials) {
                match path_auth_data.entry(path) {
                    BTreeMapEntry::Occupied(oe) => Options::path_credentials_dupe(oe.key()),
                    BTreeMapEntry::Vacant(ve) => ve.insert(auth.map(Options::normalise_credentials)),
                };
            }
        }

        let mut generate_path_auth = BTreeSet::new();
        if matches.is_present("gen-auth") {
            generate_path_auth.insert("".to_string());
        }

        if let Some(gen_path_auth) = matches.values_of("gen-path-auth") {
            for path in gen_path_auth.map(Options::normalise_path) {
                if path_auth_data.contains_key(&path) {
                    Options::path_credentials_dupe(&path);
                }

                if let Some(path) = generate_path_auth.replace(path) {
                    Options::path_credentials_dupe(&path);
                }
            }
        }

        Options {
            hosted_directory: (dir.to_string(), dir_pb.clone()),
            port: matches.value_of("port").map(u16::from_str).map(Result::unwrap),
            bind_address: matches.value_of("address").map(IpAddr::from_str).map(Result::unwrap).unwrap_or_else(|| "0.0.0.0".parse().unwrap()),
            follow_symlinks: follow_symlinks,
            sandbox_symlinks: follow_symlinks && matches.is_present("sandbox-symlinks"),
            temp_directory: {
                let (temp_s, temp_pb) = if let Some(tmpdir) = matches.value_of("temp-dir") {
                    (tmpdir.to_string(), fs::canonicalize(tmpdir).unwrap())
                } else {
                    ("$TEMP".to_string(), temp_dir())
                };
                let suffix = dir_pb.into_os_string().to_str().unwrap().replace(r"\\?\", "").replace(':', "").replace('\\', "/").replace('/', "-");
                let suffix = if suffix.len() >= 255 - (4 + 1) {
                    format!("http-{}", blake3::hash(suffix.as_bytes()).to_hex()) // avoid NAME_MAX
                } else {
                    format!("http{}{}", if suffix.starts_with('-') { "" } else { "-" }, suffix)
                };

                (format!("{}{}{}",
                         temp_s,
                         if temp_s.ends_with('/') || temp_s.ends_with('\\') {
                             ""
                         } else {
                             "/"
                         },
                         suffix),
                 temp_pb.join(suffix))
            },
            generate_listings: !matches.is_present("no-listings"),
            check_indices: !matches.is_present("no-indices"),
            strip_extensions: matches.is_present("strip-extensions"),
            try_404: matches.value_of("404").map(PathBuf::from),
            allow_writes: matches.is_present("allow-write"),
            encode_fs: !matches.is_present("no-encode"),
            encoded_filesystem_limit: matches.value_of("encoded-filesystem").and_then(|s| Options::size_parse(s.into()).ok()),
            encoded_generated_limit: matches.value_of("encoded-generated").and_then(|s| Options::size_parse(s.into()).ok()),
            encoded_prune: matches.value_of("encoded-prune").and_then(|s| Options::age_parse(s.into()).ok()),
            loglevel: matches.occurrences_of("quiet").into(),
            log_time: !matches.is_present("quiet-time"),
            log_colour: !matches.is_present("no-colour"),
            webdav: cmp::max(if matches.is_present("webdav") {
                            WebDavLevel::All
                        } else {
                            WebDavLevel::No
                        },
                        if matches.is_present("convenient-webdav") {
                            WebDavLevel::MkColMoveOnly
                        } else {
                            WebDavLevel::No
                        }),
            tls_data: matches.value_of("ssl").map(|id| ((id.to_string(), fs::canonicalize(id).unwrap()), env::var("HTTP_SSL_PASS").unwrap_or_default())),
            generate_tls: matches.is_present("gen-ssl"),
            path_auth_data: path_auth_data,
            generate_path_auth: generate_path_auth,
            proxies: matches.values_of("proxy").unwrap_or_default().map(Cow::from).map(Options::proxy_parse).map(Result::unwrap).collect(),
            proxy_redirs: matches.values_of("proxy-redir").unwrap_or_default().map(Cow::from).map(Options::proxy_parse).map(Result::unwrap).collect(),
            mime_type_overrides: matches.values_of_os("mime-type")
                .unwrap_or_default()
                .map(Cow::from)
                .map(Options::mime_type_override_parse)
                .map(Result::unwrap)
                .collect(),
            request_bandwidth: matches.value_of("request-bandwidth").map(Cow::from).map(Options::bandwidth_parse).map(Result::unwrap).unwrap_or_default(),
            additional_headers: matches.values_of("header")
                .unwrap_or_default()
                .map(Options::header_parse)
                .map(Result::unwrap)
                .collect(),
        }
    }

    fn filesystem_dir_validator(s: String, prefix: &str) -> Result<(), String> {
        fs::canonicalize(&s).map_err(|_| format!("{} \"{}\" not found", prefix, s)).and_then(|f| if f.is_dir() {
            Ok(())
        } else {
            Err(format!("{} \"{}\" not actually a directory", prefix, s))
        })
    }

    fn identity_validator(s: String) -> Result<(), String> {
        fs::canonicalize(&s).map_err(|_| format!("TLS identity file \"{}\" not found", s)).and_then(|f| if f.is_file() {
            Ok(())
        } else {
            Err(format!("TLS identity file \"{}\" not actually a file", s))
        })
    }

    fn credentials_validator(s: String) -> Result<(), String> {
        if match s.split_once(':') {
            Some((u, p)) => !u.is_empty() && !p.contains(':'),
            None => !s.is_empty(),
        } {
            Ok(())
        } else {
            Err(format!("Global authentication credentials \"{}\" need be in format \"username[:password]\"", s))
        }
    }

    fn path_credentials_validator(s: String) -> Result<(), String> {
        if Options::parse_path_credentials(&s).is_some() {
            Ok(())
        } else {
            Err(format!("Per-path authentication credentials \"{}\" need be in format \"path=[username[:password]]\"", s))
        }
    }

    fn decode_path_credentials(s: &str) -> (String, Option<&str>) {
        Options::parse_path_credentials(s).unwrap()
    }

    fn parse_path_credentials(s: &str) -> Option<(String, Option<&str>)> {
        let (path, creds) = s.split_once('=')?;

        Some((Options::normalise_path(path),
              if creds.is_empty() {
                  None
              } else {
                  if match creds.split_once(':') {
                      Some((u, p)) => u.is_empty() || p.contains(':'),
                      None => false,
                  } {
                      return None;
                  }
                  Some(creds)
              }))
    }

    fn path_credentials_dupe(path: &str) -> ! {
        ClapError {
                message: format!("Credentials for path \"/{}\" already present", path),
                kind: ClapErrorKind::ArgumentConflict,
                info: None,
            }
            .exit()
    }

    fn normalise_path(path: &str) -> String {
        let mut frags = vec![];
        for fragment in path.split(['/', '\\']) {
            match fragment {
                "" | "." => {}
                ".." => {
                    frags.pop();
                }
                _ => frags.push(fragment),
            }
        }

        let mut ret = String::with_capacity(frags.iter().map(|s| s.len()).sum::<usize>() + frags.len());
        for frag in frags {
            ret.push_str(frag);
            ret.push('/');
        }
        ret.pop();
        ret
    }

    fn normalise_credentials(creds: &str) -> String {
        if creds.ends_with(':') {
                &creds[0..creds.len() - 1]
            } else {
                creds
            }
            .to_string()
    }

    fn ipaddr_validator(s: String) -> Result<(), String> {
        IpAddr::from_str(&s).map(|_| ()).map_err(|_| format!("{} is not a valid IP address", s))
    }

    fn u16_validator(s: String) -> Result<(), String> {
        u16::from_str(&s).map(|_| ()).map_err(|_| format!("{} is not a valid port number", s))
    }

    fn size_parse<'s>(s: Cow<'s, str>) -> Result<u64, String> {
        let mut s = &s[..];
        if matches!(s.as_bytes().last(), Some(b'b' | b'B')) {
            s = &s[..s.len() - 1];
        }
        let mul: u64 = match s.as_bytes().last() {
            Some(b'k' | b'K') => 1024u64,
            Some(b'm' | b'M') => 1024u64 * 1024u64,
            Some(b'g' | b'G') => 1024u64 * 1024u64 * 1024u64,
            Some(b't' | b'T') => 1024u64 * 1024u64 * 1024u64 * 1024u64,
            Some(b'p' | b'P') => 1024u64 * 1024u64 * 1024u64 * 1024u64 * 1024u64,
            _ => 1,
        };
        if mul != 1 {
            s = &s[..s.len() - 1];
        }
        s.parse().map(|size: u64| size * mul).map_err(|e| format!("{} not a valid (optionally-K/M/G/T/P[B]-suffixed) number: {}", s, e))
    }

    fn age_parse<'s>(s: Cow<'s, str>) -> Result<u64, String> {
        let mut s = &s[..];
        let (mul, trim) = match s.as_bytes().last() {
            Some(b's') => (1, true),
            Some(b'm') => (60, true),
            Some(b'h') => (60 * 60, true),
            Some(b'd') => (60 * 60 * 24, true),
            _ => (1, false),
        };
        if trim {
            s = &s[..s.len() - 1];
        }
        s.parse().map(|age: u64| age * mul).map_err(|e| format!("{} not a valid (optionally-s/m/h/d-suffixed) number: {}", s, e))
    }

    fn proxy_parse<'s>(s: Cow<'s, str>) -> Result<(IpCidr, String), String> {
        match s.find(":") {
            None => Err(format!("{} not in HEADER-NAME:CIDR format", s)),
            Some(0) => Err(format!("{} sets invalid zero-length header", s)),
            Some(col_idx) => {
                let cidr = s[col_idx + 1..].parse().map_err(|e| format!("{} not a valid CIDR: {}", &s[col_idx + 1..], e))?;

                let mut s = s.into_owned();
                s.truncate(col_idx);
                Ok((cidr, s))
            }
        }
    }

    fn bandwidth_parse<'s>(s_orig: Cow<'s, str>) -> Result<Option<NonZeroU64>, String> {
        let s = s_orig.trim();
        let multiplier_b = s.as_bytes().get(s.len() - 1).ok_or_else(|| format!("\"{}\" bandwidth specifier empty", s_orig))?;
        let multiplier_order = match multiplier_b {
            b'k' | b'K' => 1,
            b'm' | b'M' => 2,
            b'g' | b'G' => 3,
            b't' | b'T' => 4,
            b'p' | b'P' => 5,
            b'e' | b'E' => 6,
            _ => 0,
        };
        let (multiplier, s) = match multiplier_order {
            0 => (1, s),
            mo => {
                let base: u64 = if (*multiplier_b as char).is_uppercase() {
                    1024
                } else {
                    1000
                };
                (base.pow(mo), &s[..s.len() - 1]) // No need to check, E is 2^60
            }
        };

        let number = u64::from_str(s).map_err(|e| format!("\"{}\" not band width size: {}", s, e))?;
        Ok(NonZeroU64::new(number.checked_mul(multiplier).ok_or_else(|| format!("{} * {} too big", number, multiplier))?))
    }

    fn mime_type_override_parse<'s>(s: Cow<'s, OsStr>) -> Result<(OsString, Mime), OsString> {
        let b = s.as_encoded_bytes();
        match b.iter().position(|&b| b == b':') {
            None => Err(format!("{} not in EXTENSION:MIME-TYPE format", s.to_string_lossy()).into()),
            Some(col_idx) => {
                let mime_s = str::from_utf8(&b[col_idx + 1..]).map_err(|e| format!("{} {}", s.to_string_lossy(), e))?;
                let mt = mime_s.parse().map_err(|()| format!("{} not a valid MIME type", mime_s))?;

                let mut s = s.into_owned().into_encoded_bytes();
                s.truncate(col_idx);
                Ok((unsafe { OsString::from_encoded_bytes_unchecked(s) }, mt))
            }
        }
    }

    fn header_parse(s: &str) -> Result<(String, Vec<u8>), String> {
        s.split_once(':')
            .and_then(|(hn, mut hd)| {
                hd = hd.trim_start();
                if !hn.is_empty() && !hd.is_empty() {
                    Some((hn, hd))
                } else {
                    None
                }
            })
            .map(|(hn, hd)| (hn.to_string(), hd.as_bytes().to_vec()))
            .ok_or_else(|| format!("\"{}\" invalid format", s))
    }
}
