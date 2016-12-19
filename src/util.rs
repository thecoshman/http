//! Module containing various utility functions.


use iron::Url;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::borrow::Cow;
use url::percent_encoding;
use time::{self, Duration, Tm};


/// The generic HTML page to use as response to errors.
pub static ERROR_HTML: &'static str = include_str!("../assets/error.html");

/// The HTML page to use as template for a requested directory's listing.
pub static DIRECTORY_LISTING_HTML: &'static str = include_str!("../assets/directory_listing.html");

/// The port to start scanning from if no ports were given.
pub static PORT_SCAN_LOWEST: u16 = 8000;

/// The port to end scanning at if no ports were given.
pub static PORT_SCAN_HIGHEST: u16 = 9999;

/// The app name and version to use with User-Agent or Server response header.
pub static USER_AGENT: &'static str = concat!("http/", env!("CARGO_PKG_VERSION"));


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
/// println!(html_response(NOT_IMPLEMENTED_HTML, &["<p>Abolish the burgeoisie!</p>"]));
/// ```
pub fn html_response<S: AsRef<str>>(data: &str, format_strings: &[S]) -> String {
    format_strings.iter().enumerate().fold(data.to_string(), |d, (i, s)| d.replace(&format!("{{{}}}", i), s.as_ref()))
}

/// Return the path part of the URL.
///
/// # Example
///
/// ```
/// # extern crate iron;
/// # extern crate https;
/// # use iron::Url;
/// # use https::util::url_path;
/// let url = Url::parse("127.0.0.1:8000/capitalism/русский/");
/// assert_eq!(url_path(&url), "capitalism/русский/");
/// ```
pub fn url_path(url: &Url) -> String {
    url.path().into_iter().fold("".to_string(),
                                |cur, pp| format!("{}/{}", cur, percent_decode(pp).unwrap_or(Cow::Borrowed("<incorrect UTF8>"))))[1..]
        .to_string()
}

/// Decode a percent-encoded string (like a part of a URL).
///
/// # Example
///
/// ```
/// # use https::util::percent_decode;
/// # use std::borrow::Cow;
/// assert_eq!(percent_decode("%D0%B0%D1%81%D0%B4%D1%84%20fdsa"), Some(Cow::Owned("асдф fdsa".to_string())));
/// assert_eq!(percent_decode("%D0%D1%81%D0%B4%D1%84%20fdsa"), None);
/// ```
pub fn percent_decode(s: &str) -> Option<Cow<str>> {
    percent_encoding::percent_decode(s.as_bytes()).decode_utf8().ok()
}

/// Get the timestamp of the file's last modification as a `time::Tm`.
pub fn file_time_modified(f: &Path) -> Tm {
    match f.metadata().unwrap().modified().unwrap().elapsed() {
        Ok(dur) => time::now() - Duration::from_std(dur).unwrap(),
        Err(ste) => time::now() + Duration::from_std(ste.duration()).unwrap(),
    }
}
