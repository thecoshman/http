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


use clap::{AppSettings, Arg, App};
use std::path::PathBuf;
use std::env::temp_dir;
use std::str::FromStr;
use std::fs;


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// The directory to host.
    pub hosted_directory: (String, PathBuf),
    /// The port to host on. Default: first free port from 8000 up
    pub port: Option<u16>,
    /// Whether to allow symlinks to be requested. Default: true
    pub follow_symlinks: bool,
    /// The temp directory to write to before copying to hosted directory. Default: `None`
    pub temp_directory: Option<(String, PathBuf)>,
    /// Whether to check for index files in served directories before serving a listing. Default: true
    pub check_indices: bool,
    /// Whether to allow write operations. Default: false
    pub allow_writes: bool,
    /// Whether to encode filesystem files. Default: true
    pub encode_fs: bool,
    /// Data for HTTPS, identity file and password. Default: `None`
    pub tls_data: Option<((PathBuf, String), String)>,
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
            .arg(Arg::from_usage("-w --allow-write 'Allow for write operations. Default: false'"))
            .arg(Arg::from_usage("-i --no-indices 'Always generate dir listings even if index files are available. Default: false'"))
            .arg(Arg::from_usage("-e --no-encode 'Do not encode filesystem files. Default: false'"))
            .arg(Arg::from_usage("--ssl [TLS_IDENTITY_PASSWIRD] 'Data for HTTPS, identity file and password. In the form of identity_file,password'")
                .validator(Options::identity_validator))
            .get_matches();

        let w = matches.is_present("allow-write");
        let e = !matches.is_present("no-encode");
        let dir = matches.value_of("DIR").unwrap_or(".");
        let dir_pb = fs::canonicalize(dir).unwrap();
        Options {
            hosted_directory: (dir.to_string(), dir_pb.clone()),
            port: matches.value_of("port").map(u16::from_str).map(Result::unwrap),
            follow_symlinks: !matches.is_present("no-follow-symlinks"),
            temp_directory: if w || e {
                let (temp_s, temp_pb) = if let Some(tmpdir) = matches.value_of("temp-dir") {
                    (tmpdir.to_string(), fs::canonicalize(tmpdir).unwrap())
                } else {
                    ("$TEMP".to_string(), temp_dir())
                };
                let suffix = dir_pb.into_os_string().to_str().unwrap().replace(r"\\?\", "").replace(':', "").replace('\\', "/").replace('/', "-");
                let suffix = format!("http{}{}", if suffix.starts_with('-') { "" } else { "-" }, suffix);

                Some((format!("{}{}{}",
                              temp_s,
                              if temp_s.ends_with("/") || temp_s.ends_with(r"\") {
                                  ""
                              } else {
                                  "/"
                              },
                              suffix),
                      temp_pb.join(suffix)))
            } else {
                None
            },
            check_indices: !matches.is_present("no-indices"),
            allow_writes: w,
            encode_fs: e,
            tls_data: if let Some(idpwd) = matches.value_of("ssl") {
                let comma_idx = idpwd.find(',').unwrap();
                Some(((fs::canonicalize(&idpwd[0..comma_idx]).unwrap(), idpwd[0..comma_idx].to_string()), idpwd[comma_idx + 1..].to_string()))
            } else {
                None
            },
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
        let comma_idx = try!(s.find(',').ok_or_else(|| format!("{} is not in the form 'identity_file,password'", s)));
        fs::canonicalize(&s[0..comma_idx]).map_err(|_| format!("TLS identity file \"{}\" not found", &s[0..comma_idx])).and_then(|f| if f.is_file() {
            Ok(())
        } else {
            Err(format!("TLS identity file \"{}\" not actualy a file", &s[0..comma_idx]))
        })
    }

    fn u16_validator(s: String) -> Result<(), String> {
        u16::from_str(&s).map(|_| ()).map_err(|_| format!("{} is not a valid port number", s))
    }
}
