//! Module containing various utility functions.


mod content_encoding;

use base64;
use std::f64;
use std::cmp;
use iron::Url;
use std::fs::File;
use std::path::Path;
use std::borrow::Cow;
use url::percent_encoding;
use std::collections::HashMap;
use time::{self, Duration, Tm};
use std::io::{BufReader, BufRead};
use mime_guess::get_mime_type_str;

pub use self::content_encoding::*;


/// The generic HTML page to use as response to errors.
pub static ERROR_HTML: &'static str = include_str!("../../assets/error.html");

/// The HTML page to use as template for a requested directory's listing.
pub static DIRECTORY_LISTING_HTML: &'static str = include_str!("../../assets/directory_listing.html");

lazy_static! {
    /// Collection of data to be injected into generated responses.
    pub static ref ASSETS: HashMap<&'static str, Cow<'static, str>> = {
        let mut ass = HashMap::with_capacity(8);
        ass.insert("favicon",
            Cow::Owned(format!("data:{};base64,{}", get_mime_type_str("ico").unwrap(), base64::encode(include_bytes!("../../assets/favicon.ico")))));
        ass.insert("dir_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               base64::encode(include_bytes!("../../assets/icons/directory_icon.gif")))));
        ass.insert("file_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               base64::encode(include_bytes!("../../assets/icons/file_icon.gif")))));
        ass.insert("file_binary_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               base64::encode(include_bytes!("../../assets/icons/file_binary_icon.gif")))));
        ass.insert("file_image_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               base64::encode(include_bytes!("../../assets/icons/file_image_icon.gif")))));
        ass.insert("file_text_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               base64::encode(include_bytes!("../../assets/icons/file_text_icon.gif")))));
        ass.insert("back_arrow_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               base64::encode(include_bytes!("../../assets/icons/back_arrow_icon.gif")))));
        ass.insert("drag_drop", Cow::Borrowed(include_str!("../../assets/drag_drop.js")));
        ass
    };
}

/// The port to start scanning from if no ports were given.
pub static PORT_SCAN_LOWEST: u16 = 8000;

/// The port to end scanning at if no ports were given.
pub static PORT_SCAN_HIGHEST: u16 = 9999;

/// The app name and version to use with User-Agent or Server response header.
pub static USER_AGENT: &'static str = concat!("http/", env!("CARGO_PKG_VERSION"));

/// Index file extensions to look for if `-i` was not specified.
pub static INDEX_EXTENSIONS: &'static [&'static str] = &["html", "htm", "shtml"];


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

/// Check if the specified file is to be considered "binary".
///
/// Basically checks is a file is UTF-8.
///
/// # Examples
///
/// ```
/// # use https::util::file_binary;
/// # #[cfg(target_os = "windows")]
/// # assert!(file_binary("target/debug/http.exe"));
/// # #[cfg(not(target_os = "windows"))]
/// assert!(file_binary("target/debug/http"));
/// assert!(!file_binary("Cargo.toml"));
/// ```
pub fn file_binary<P: AsRef<Path>>(path: P) -> bool {
    File::open(path).and_then(|f| BufReader::new(f).read_line(&mut String::new())).is_err()
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
    ASSETS.iter().fold(format_strings.iter().enumerate().fold(data.to_string(), |d, (i, s)| d.replace(&format!("{{{}}}", i), s.as_ref())),
                       |d, (k, v)| d.replace(&format!("{{{}}}", k), v))
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
    let path = url.path();
    if path == [""] {
        "/".to_string()
    } else {
        path.into_iter().fold("".to_string(),
                              |cur, pp| format!("{}/{}", cur, percent_decode(pp).unwrap_or(Cow::Borrowed("<incorrect UTF8>"))))[1..]
            .to_string()
    }
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

/// Check, whether, in any place of the path, a file is treated like a directory.
///
/// A file is treated like a directory when it is treated as if it had a subpath, e.g., given:
///
/// ```sh
/// tree .
/// | dir0
/// | dir1
///   | file01
/// ```
///
/// This function would return true for `./dir1/file01/file010`, `./dir1/file01/dir010/file0100`, etc., but not
/// for `./dir0/file00`, `./dir0/dir00/file000`, `./dir1/file02/`, `./dir1/dir010/file0100`.
pub fn detect_file_as_dir(mut p: &Path) -> bool {
    while let Some(pnt) = p.parent() {
        if pnt.is_file() {
            return true;
        }

        p = pnt;
    }

    false
}

/// Check if a path refers to a symlink in a way that also works on Windows.
pub fn is_symlink<P: AsRef<Path>>(p: P) -> bool {
    p.as_ref().read_link().is_ok()
}

/// Construct string representing a human-readable size.
///
/// Stolen, adapted and inlined from [fielsize.js](http://filesizejs.com).
pub fn human_readable_size(s: u64) -> String {
    lazy_static! {
        static ref LN_KIB: f64 = 1024f64.log(f64::consts::E);
    }

    if s == 0 {
        "0 B".to_string()
    } else {
        let num = s as f64;
        let exp = cmp::min(cmp::max((num.log(f64::consts::E) / *LN_KIB) as i32, 0), 8);

        let val = num / 2f64.powi(exp * 10);

        if exp > 0 {
                (val * 10f64).round() / 10f64
            } else {
                val.round()
            }
            .to_string() + " " + ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"][cmp::max(exp, 0) as usize]
    }
}
