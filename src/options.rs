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
use std::collections::BTreeSet;
use std::env::{self, temp_dir};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::str::FromStr;
use std::borrow::Cow;
use iron::mime::Mime;
use std::net::IpAddr;
use regex::Regex;
use cidr::IpCidr;
use std::fs;


lazy_static! {
    static ref CREDENTIALS_REGEX: Regex = Regex::new("[^:]+(?::[^:]+)?").unwrap();
    static ref PATH_CREDENTIALS_REGEX: Regex = Regex::new("(.+)=([^:]+(?::[^:]+)?)?").unwrap();
}


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
    /// Whether to allow write operations. Default: false
    pub allow_writes: bool,
    /// Whether to encode filesystem files. Default: true
    pub encode_fs: bool,
    /// How much to suppress output
    ///
    ///   * >= 1 – suppress serving status lines ("IP was served something")
    ///   * >= 2 – suppress startup except for auth data, if present
    ///   * >= 3 – suppress all startup messages
    pub loglevel: LogLevel,
    /// Whether to colourise the log output. Default: `true`
    pub log_colour: bool,
    /// Whether to handle WebDAV requests. Default: false
    pub webdav: bool,
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
    /// Extension -> MIME type mapping overrides; empty string for no extension
    pub mime_type_overrides: BTreeMap<String, Mime>,
    /// Max amount of data per second each request is allowed to return. Default: `None`
    pub request_bandwidth: Option<NonZeroU64>,
}

impl Options {
    /// Parse `env`-wide command-line arguments into an `Options` instance
    pub fn parse() -> Options {
        let matches = App::new("http")
            .version(crate_version!())
            .author(crate_authors!("\n"))
            .about(crate_description!())
            .setting(AppSettings::ColoredHelp)
            .arg(Arg::from_usage("[DIR] 'Directory to host. Default: current working directory'")
                .validator(|s| Options::filesystem_dir_validator(s, "Directory to host")))
            .arg(Arg::from_usage("-p --port [port] 'Port to use. Default: first free port from 8000 up'").validator(Options::u16_validator))
            .arg(Arg::from_usage("-a --address [address] 'Address to bind to. Default: 0.0.0.0'").validator(Options::ipaddr_validator))
            .arg(Arg::from_usage("-t --temp-dir [temp] 'Temporary directory. Default: $TEMP'")
                .validator(|s| Options::filesystem_dir_validator(s, "Temporary directory")))
            .arg(Arg::from_usage("-s --no-follow-symlinks 'Don't follow symlinks. Default: false'"))
            .arg(Arg::from_usage("-r --sandbox-symlinks 'Restrict/sandbox where symlinks lead to only the direct descendants of the hosted directory. \
                                  Default: false'"))
            .arg(Arg::from_usage("-w --allow-write 'Allow for write operations. Default: false'"))
            .arg(Arg::from_usage("-l --no-listings 'Never generate dir listings. Default: false'"))
            .arg(Arg::from_usage("-i --no-indices 'Do not automatically use index files. Default: false'"))
            .arg(Arg::from_usage("-e --no-encode 'Do not encode filesystem files. Default: false'"))
            .arg(Arg::from_usage("-x --strip-extensions 'Allow stripping index extentions from served paths. Default: false'"))
            .arg(Arg::from_usage("-q --quiet... 'Suppress increasing amounts of output'"))
            .arg(Arg::from_usage("-c --no-colour 'Don't colourise the log output'"))
            .arg(Arg::from_usage("-d --webdav 'Handle WebDAV requests. Default: false'"))
            .arg(Arg::from_usage("--ssl [TLS_IDENTITY] 'Data for HTTPS, identity file. Password in HTTP_SSL_PASS env var, otherwise empty'")
                .validator(Options::identity_validator))
            .arg(Arg::from_usage("--gen-ssl 'Generate a one-off TLS certificate'").conflicts_with("ssl"))
            .arg(Arg::from_usage("--auth [USERNAME[:PASSWORD]] 'Data for global authentication'").validator(Options::credentials_validator))
            .arg(Arg::from_usage("--gen-auth 'Generate a one-off username:password set for global authentication'").conflicts_with("auth"))
            .arg(Arg::from_usage("--path-auth [PATH=[USERNAME[:PASSWORD]]]... 'Data for authentication under PATH'")
                .validator(Options::path_credentials_validator))
            .arg(Arg::from_usage("--gen-path-auth [PATH]... 'Generate a one-off username:password set for authentication under PATH'"))
            .arg(Arg::from_usage("--proxy [HEADER-NAME:CIDR]... 'Treat HEADER-NAME as proxy forwarded-for header when request comes from CIDR'")
                .validator(|s| Options::proxy_parse(s.into()).map(|_| ())))
            .arg(Arg::from_usage("-m --mime-type [EXTENSION:MIME-TYPE]... 'Always return MIME-TYPE for files with EXTENSION'")
                .validator(|s| Options::mime_type_override_parse(s.into()).map(|_| ())))
            .arg(Arg::from_usage("--request-bandwidth [BYTES] 'Limit each request to returning BYTES per second, or 0 for unlimited. Default: 0'")
                .validator(|s| Options::bandwidth_parse(s.into()).map(|_| ())))
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
                let suffix = format!("http{}{}", if suffix.starts_with('-') { "" } else { "-" }, suffix);

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
            allow_writes: matches.is_present("allow-write"),
            encode_fs: !matches.is_present("no-encode"),
            loglevel: matches.occurrences_of("quiet").into(),
            log_colour: !matches.is_present("no-colour"),
            webdav: matches.is_present("webdav"),
            tls_data: matches.value_of("ssl").map(|id| ((id.to_string(), fs::canonicalize(id).unwrap()), env::var("HTTP_SSL_PASS").unwrap_or_default())),
            generate_tls: matches.is_present("gen-ssl"),
            path_auth_data: path_auth_data,
            generate_path_auth: generate_path_auth,
            proxies: matches.values_of("proxy").unwrap_or_default().map(Cow::from).map(Options::proxy_parse).map(Result::unwrap).collect(),
            mime_type_overrides: matches.values_of("mime-type")
                .unwrap_or_default()
                .map(Cow::from)
                .map(Options::mime_type_override_parse)
                .map(Result::unwrap)
                .collect(),
            request_bandwidth: matches.value_of("request-bandwidth").map(Cow::from).map(Options::bandwidth_parse).map(Result::unwrap).unwrap_or_default(),
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
        if CREDENTIALS_REGEX.is_match(&s) {
            Ok(())
        } else {
            Err(format!("Global authentication credentials \"{}\" need be in format \"username[:password]\"", s))
        }
    }

    fn path_credentials_validator(s: String) -> Result<(), String> {
        if PATH_CREDENTIALS_REGEX.is_match(&s) {
            Ok(())
        } else {
            Err(format!("Per-path authentication credentials \"{}\" need be in format \"path=[username[:password]]\"", s))
        }
    }

    fn decode_path_credentials(s: &str) -> (String, Option<&str>) {
        let creds = PATH_CREDENTIALS_REGEX.captures(s).unwrap();

        (Options::normalise_path(&creds[1]), creds.get(2).map(|m| m.as_str()))
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
        for fragment in path.split(|c| c == '/' || c == '\\') {
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

    fn mime_type_override_parse<'s>(s: Cow<'s, str>) -> Result<(String, Mime), String> {
        match s.find(":") {
            None => Err(format!("{} not in EXTENSION:MIME-TYPE format", s)),
            Some(col_idx) => {
                let mt = s[col_idx + 1..].parse().map_err(|()| format!("{} not a valid MIME type", &s[col_idx + 1..]))?;

                let mut s = s.into_owned();
                s.truncate(col_idx);
                Ok((s, mt))
            }
        }
    }
}
