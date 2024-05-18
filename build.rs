extern crate embed_resource;
extern crate base64;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern crate cc;

use std::env;
use std::io::Write;
use std::path::Path;
use std::fs::{self, File};
use base64::display::Base64Display;
use std::collections::{BTreeMap, BTreeSet};



fn main() {
    htmls();
    extensions();

    embed_resource::compile("http-manifest.rc");

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
    cc::Build::new().file("build-ioctl.c").define("_GNU_SOURCE", "1").compile("http-ioctl");
}


fn assets() -> Vec<(&'static str, String)> {
    let mut assets = Vec::new();
    for (key, mime, file) in
        [("favicon", "image/png", "assets/favicon.png"),
         ("dir_icon", "image/gif", "assets/icons/directory.gif"),
         ("file_icon", "image/gif", "assets/icons/file.gif"),
         ("file_binary_icon", "image/gif", "assets/icons/file_binary.gif"),
         ("file_image_icon", "image/gif", "assets/icons/file_image.gif"),
         ("file_text_icon", "image/gif", "assets/icons/file_text.gif"),
         ("back_arrow_icon", "image/gif", "assets/icons/back_arrow.gif"),
         ("new_dir_icon", "image/gif", "assets/icons/new_directory.gif"),
         ("delete_file_icon", "image/png", "assets/icons/delete_file.png"),
         ("rename_icon", "image/gif", "assets/icons/rename.gif"),
         ("confirm_icon", "image/png", "assets/icons/confirm.png")] {
        println!("cargo:rerun-if-changed={}", file);
        assets.push((key,
                     format!("data:{};base64,{}",
                             mime,
                             Base64Display::with_config(&fs::read(file).unwrap()[..], base64::STANDARD))));
    }
    for (key, file) in [("date", "assets/date.js"),
                        ("manage", "assets/manage.js"),
                        ("manage_mobile", "assets/manage_mobile.js"),
                        ("manage_desktop", "assets/manage_desktop.js"),
                        ("upload", "assets/upload.js"),
                        ("adjust_tz", "assets/adjust_tz.js")] {
        println!("cargo:rerun-if-changed={}", file);
        assets.push((key, fs::read_to_string(file).unwrap()));
    }
    assets
}

fn htmls() {
    let assets = assets();
    for html in ["error.html", "directory_listing.html", "directory_listing_mobile.html"] {
        println!("cargo:rerun-if-changed=assets/{}", html);

        let with_assets = assets.iter().fold(fs::read_to_string(format!("assets/{}", html)).unwrap(),
                                             |d, (k, v)| d.replace(&format!("{{{}}}", k), v));

        let mut arguments = BTreeMap::new();
        for i in 0.. {
            let len_pre = arguments.len();
            arguments.extend(with_assets.match_indices(&format!("{{{}}}", i)).map(|(start, s)| (start, (s.len(), i))));
            if arguments.len() == len_pre {
                break;
            }
        }

        let mut out = File::create(Path::new(&env::var("OUT_DIR").unwrap()).join(format!("{}.rs", html))).unwrap();
        writeln!(out, "&[").unwrap();
        let mut idx = 0;
        for (start, (len, argi)) in arguments {
            writeln!(out, "PreparsedHtml::Literal({:?}),", &with_assets[idx..start]).unwrap();
            writeln!(out, "PreparsedHtml::Argument({}),", argi).unwrap();
            idx = start + len;
        }
        writeln!(out, "PreparsedHtml::Literal({:?}),", &with_assets[idx..]).unwrap();
        writeln!(out, "]").unwrap();
    }
}


fn extensions() {
    println!("cargo:rerun-if-changed={}", "assets/encoding_blacklist");
    let mut out = File::create(Path::new(&env::var("OUT_DIR").unwrap()).join("extensions.rs")).unwrap();

    let raw = fs::read_to_string("assets/encoding_blacklist").unwrap();
    let mut exts = BTreeMap::new();
    for ext in raw.split('\n').map(str::trim).filter(|s| !s.is_empty() && !s.starts_with('#')) {
        exts.entry(ext.len()).or_insert(BTreeSet::new()).insert(ext);
    }
    writeln!(out, "pub fn extension_is_blacklisted(ext: &OsStr) -> bool {{").unwrap();
    writeln!(out, "#[cfg(not(target_os = \"windows\"))] use std::os::unix::ffi::OsStrExt;").unwrap();


    write!(out, "if !matches!(ext.len(),").unwrap();
    for (i, len) in exts.keys().enumerate() {
        write!(out, " {} {}", if i == 0 { "" } else { "|" }, len).unwrap();
    }
    writeln!(out, ") {{ return false; }}").unwrap();

    let maxlen = exts.keys().max().unwrap();
    writeln!(out,
             r#"
let mut buf = [0u8; {}];
#[cfg(not(target_os = "windows"))]
let bytes = ext.as_bytes();
#[cfg(target_os = "windows")]
let bytes = ext.as_encoded_bytes();
for (i, b) in bytes.iter().enumerate() {{
if !b.is_ascii_alphanumeric() {{
    return false;
}}
buf[i] = b.to_ascii_lowercase();
}}
let lcase = &buf[0..ext.len()];
"#,
             maxlen)
        .unwrap();

    write!(out, "matches!(lcase,").unwrap();
    for (i, ext) in exts.values().flatten().enumerate() {
        write!(out, " {} b{:?}", if i == 0 { "" } else { "|" }, ext).unwrap();
    }
    writeln!(out, ")").unwrap();

    writeln!(out, "}}").unwrap();
}
