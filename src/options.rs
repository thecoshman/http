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
use std::path::PathBuf;
use std::str::FromStr;
use regex::Regex;
use std::fs;


lazy_static! {
    static ref CREDENTIALS_REGEX: Regex = Regex::new("[^:]+(?::[^:]+)?").unwrap();
    static ref PATH_CREDENTIALS_REGEX: Regex = Regex::new("(.+)=([^:]+(?::[^:]+)?)?").unwrap();
}


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// The directory to host.
    pub hosted_directory: (String, PathBuf),
    /// The port to host on. Default: first free port from 8000 up
    pub port: Option<u16>,
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
    /// Whether to check for index files in served directories before serving a listing. Default: true
    pub check_indices: bool,
    /// Whether to allow write operations. Default: false
    pub allow_writes: bool,
    /// Whether to encode filesystem files. Default: true
    pub encode_fs: bool,
    /// Data for HTTPS, identity file and password. Default: `None`
    pub tls_data: Option<((String, PathBuf), String)>,
    /// Whether to generate a one-off certificate. Default: false
    pub generate_tls: bool,
    /// Data for per-path authentication, in the form `username[:password]`, or `None` to explicitly disable
    pub path_auth_data: BTreeMap<String, Option<String>>,
    /// Paths for which to generate auth data
    pub generate_path_auth: BTreeSet<String>,
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
            .arg(Arg::from_usage("-t --temp-dir [temp] 'Temporary directory. Default: $TEMP'")
                .validator(|s| Options::filesystem_dir_validator(s, "Temporary directory")))
            .arg(Arg::from_usage("-s --no-follow-symlinks 'Don't follow symlinks. Default: false'"))
            .arg(Arg::from_usage("-r --sandbox-symlinks 'Restrict/sandbox where symlinks lead to only the direct descendants of the hosted directory. \
                                  Default: false'"))
            .arg(Arg::from_usage("-w --allow-write 'Allow for write operations. Default: false'"))
            .arg(Arg::from_usage("-i --no-indices 'Always generate dir listings even if index files are available. Default: false'"))
            .arg(Arg::from_usage("-e --no-encode 'Do not encode filesystem files. Default: false'"))
            .arg(Arg::from_usage("--ssl [TLS_IDENTITY] 'Data for HTTPS, identity file. Password in HTTP_SSL_PASS env var, otherwise empty'")
                .validator(Options::identity_validator))
            .arg(Arg::from_usage("--gen-ssl 'Generate a one-off TLS certificate'").conflicts_with("ssl"))
            .arg(Arg::from_usage("--auth [USERNAME[:PASSWORD]] 'Data for global authentication'").validator(Options::credentials_validator))
            .arg(Arg::from_usage("--gen-auth 'Generate a one-off username:password set for global authentication'").conflicts_with("auth"))
            .arg(Arg::from_usage("--path-auth [PATH=[USERNAME[:PASSWORD]]]... 'Data for authentication under PATH'")
                .validator(Options::path_credentials_validator))
            .arg(Arg::from_usage("--gen-path-auth [PATH]... 'Generate a one-off username:password set for authentication under PATH'"))
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
            check_indices: !matches.is_present("no-indices"),
            allow_writes: matches.is_present("allow-write"),
            encode_fs: !matches.is_present("no-encode"),
            tls_data: matches.value_of("ssl").map(|id| ((id.to_string(), fs::canonicalize(id).unwrap()), env::var("HTTP_SSL_PASS").unwrap_or(String::new()))),
            generate_tls: matches.is_present("gen-ssl"),
            path_auth_data: path_auth_data,
            generate_path_auth: generate_path_auth,
        }
    }

    fn filesystem_dir_validator(s: String, prefix: &str) -> Result<(), String> {
        fs::canonicalize(&s).map_err(|_| format!("{} \"{}\" not found", prefix, s)).and_then(|f| if f.is_dir() {
            Ok(())
        } else {
            Err(format!("{} \"{}\" not actualy a directory", prefix, s))
        })
    }

    fn identity_validator(s: String) -> Result<(), String> {
        fs::canonicalize(&s).map_err(|_| format!("TLS identity file \"{}\" not found", s)).and_then(|f| if f.is_file() {
            Ok(())
        } else {
            Err(format!("TLS identity file \"{}\" not actualy a file", s))
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

    fn u16_validator(s: String) -> Result<(), String> {
        u16::from_str(&s).map(|_| ()).map_err(|_| format!("{} is not a valid port number", s))
    }
}
