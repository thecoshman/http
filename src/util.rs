//! Module containing various utility functions.


use std::fs::File;
use std::io::Read;
use std::path::Path;


/// The HTML page to use as response when hitting an unimplemented corner of the server.
///
/// To be used with 501 Not Implemented status,
pub static NOT_IMPLEMENTED_HTML: &'static str = include_str!("../assets/501.html");

/// The HTML page to use as response when a non-existant file was requested.
///
/// To be used with 404 Not Found status,
pub static NOT_FOUND_HTML: &'static str = include_str!("../assets/404.html");

/// The port to start scanning from if no ports were given.
pub static PORT_SCAN_LOWEST: u16 = 8000;

/// The port to end scanning at if no ports were given.
pub static PORT_SCAN_HIGHEST: u16 = 9999;


/// Uppercase the first character of the supplied string.
///
/// Based on http://stackoverflow.com/a/38406885/2851815
///
/// # Examples
///
/// ```
/// # use https::util::uppercase_first;
/// assert_eq!(uppercase_first("abolish"), "Abolish".to_string());
/// ```
pub fn uppercase_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Check if the specified file contains the specified byte.
///
/// # Examples
///
/// ```
/// # use https::util::file_contains;
/// # #[cfg(target_os = "windows")]
/// # assert!(file_contains("target/debug/http.exe", 0));
/// # #[cfg(not(target_os = "windows"))]
/// assert!(file_contains("target/debug/http", 0));
/// assert!(!file_contains("Cargo.toml", 0));
/// ```
pub fn file_contains<P: AsRef<Path>>(path: P, byte: u8) -> bool {
    if let Ok(mut f) = File::open(path) {
        let mut buf = [0u8; 1024];

        while let Ok(read) = f.read(&mut buf) {
            if buf[..read].contains(&byte) {
                return true;
            }

            if read < buf.len() {
                break;
            }
        }
    }

    false
}
