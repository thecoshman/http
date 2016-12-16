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


use clap::{AppSettings, App, Arg};
use std::path::PathBuf;
use std::str::FromStr;
use std::fs;


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// The directory to host.
    pub hosted_directory: (String, PathBuf),
    /// The port to host on. Default: first free port from 8000 up
    pub port: Option<u16>,
    /// Whether to allow symlinks to be requested. Default: false
    pub follow_symlinks: bool,
}

impl Options {
    /// Parse `env`-wide command-line arguments into an `Options` instance
    pub fn parse() -> Options {
        let matches = App::new("http")
            .version(crate_version!())
            .author(crate_authors!())
            .setting(AppSettings::ColoredHelp)
            .about("Host These Things Please - a basic HTTP server for hosting a folder fast and simply")
            .arg(Arg::from_usage("[DIR] 'Directory to host. Default: current working directory'").validator(Options::filesystem_dir_validator))
            .arg(Arg::from_usage("-p --port [port] 'Port to use. Default: first free port from 8000 up'").validator(Options::u16_validator))
            .arg(Arg::from_usage("-s --follow-symlinks 'Follow symlinks. Default: false'"))
            .get_matches();

        let dir = matches.value_of("DIR").unwrap_or(".");
        Options {
            hosted_directory: (dir.to_string(), fs::canonicalize(dir).unwrap()),
            port: matches.value_of("port").map(u16::from_str).map(Result::unwrap),
            follow_symlinks: matches.is_present("follow-symlinks"),
        }
    }

    fn filesystem_dir_validator(s: String) -> Result<(), String> {
        fs::canonicalize(&s).map_err(|_| format!("Directory to host \"{}\" not found", s)).and_then(|f| if f.is_dir() {
            Ok(())
        } else {
            Err(format!("Directory to host \"{}\" not actualy a directory", s))
        })
    }

    fn u16_validator(s: String) -> Result<(), String> {
        u16::from_str(&s).map(|_| ()).map_err(|_| format!("{} is not a valid port number", s))
    }
}
