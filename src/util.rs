//! Module containing various utility functions.


use std::fs::File;
use std::io::Read;
use std::path::Path;


/// The generic HTML page to use as response to errors.
pub static ERROR_HTML: &'static str = include_str!("../assets/error.html");

/// The HTML page to use as template for a requested directory's listing.
pub static DIRECTORY_LISTING_HTML: &'static str = include_str!("../assets/directory_listing.html");

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

/// Fill out an HTML template.
///
/// All fields must be addressed even if formatted to be empty.
///
/// # Examples
///
/// ```
/// # use https::util::{html_response, NOT_IMPLEMENTED_HTML};
/// println!(html_response(NOT_IMPLEMENTED_HTML, vec!["<p>Abolish the burgeoisie!</p>".to_string()]));
/// ```
pub fn html_response(data: &str, format_strings: Vec<String>) -> String {
    format_strings.iter().enumerate().fold(data.to_string(), |d, (i, ref s)| d.replace(&format!("{{{}}}", i), s))
}
