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


use clap::{App, Arg, AppSettings};
use std::path::PathBuf;
use std::fs;


/// Representation of the application's all configurable values.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Options {
    /// The directory to host.
    pub hosted_directory: (String, PathBuf),
    /// The port to host on. Default: first free one from 8000 up
    pub port: u16,
}

impl Options {
    /// Parse `env`-wide command-line arguments into an `Options` instance
    pub fn parse() -> Options {
        let matches = App::new("http")
            .version(crate_version!())
            .author(crate_authors!())
            .setting(AppSettings::ColoredHelp)
            .setting(AppSettings::VersionlessSubcommands)
            .about("Host These Things Please - a basic HTTP server for hosting a folder fast and simply")
            .arg(Arg::from_usage("<DIR> 'Directory to host'").validator(Options::filesystem_dir_validator))
            .get_matches();

        let dir = matches.value_of("DIR").unwrap();
        Options {
            hosted_directory: (dir.to_string(), fs::canonicalize(dir).unwrap()),
            port: 8000,
        }
    }

    fn filesystem_dir_validator(s: String) -> Result<(), String> {
        fs::canonicalize(&s).map_err(|_| format!("Directory to host \"{}\" not found", s)).and_then(|f| if f.is_dir() {
            Ok(())
        } else {
            Err(format!("Directory to host \"{}\" not actualy a directory", s))
        })
    }
}
