extern crate embed_resource;
extern crate base64;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern crate cc;

use std::env;
use std::path::Path;
use std::fs::{self, File};
use base64::display::Base64Display;
use std::io::{BufReader, BufRead, Write};
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
    {
        println!("cargo:rerun-if-changed=Cargo.toml");
        assets.push(("generator",
                     format!("http {}",
                             BufReader::new(File::open("Cargo.toml").unwrap()).lines().flatten().find(|l| l.starts_with("version = ")).unwrap()
                                 ["version = ".len()..]
                                 .trim_matches('"'))));
    }
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
         ("confirm_icon", "image/gif", "assets/icons/confirm.gif")] {
        println!("cargo:rerun-if-changed={}", file);
        assets.push((key,
                     format!("data:{};base64,{}",
                             mime,
                             Base64Display::with_config(&fs::read(file).unwrap()[..], base64::STANDARD))));
    }
    for (key, file) in [("manage", "assets/manage.js"),
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

        let with_assets = assets.iter()
            .fold(fs::read_to_string(format!("assets/{}", html)).unwrap(),
                  |d, (k, v)| d.replace(&format!("{{{}}}", k), v))
            .lines()
            .flat_map(|l| [l.trim(), "\n"])
            .collect::<String>();

        let mut arguments = BTreeMap::new();
        for i in 0.. {
            let len_pre = arguments.len();
            arguments.extend(with_assets.match_indices(&format!("{{{}}}", i)).map(|(start, s)| (start, (s.len(), i))));
            if arguments.len() == len_pre {
                break;
            }
        }

        let mut data = Vec::new();
        let mut argsused = BTreeMap::<u32, u8>::new();
        let mut idx = 0;
        for (start, (len, argi)) in arguments {
            if with_assets[idx..start].len() != 0 {
                data.push(Ok(&with_assets[idx..start]));
            }
            data.push(Err(argi));
            *argsused.entry(argi).or_default() += 1;
            idx = start + len;
        }


        let mut out = File::create(Path::new(&env::var("OUT_DIR").unwrap()).join(format!("{}.rs", html))).unwrap();
        write!(&mut out, "pub fn {}<", html.replace('.', "_")).unwrap();
        for (arg, nused) in &argsused {
            if *nused == 1 {
                write!(&mut out, "T{}: HtmlResponseElement, ", arg).unwrap();
            } else {
                write!(&mut out, "T{}: HtmlResponseElement + Copy, ", arg).unwrap();
            }
        }
        write!(&mut out, ">(").unwrap();
        for (arg, _) in &argsused {
            write!(&mut out, "a{}: T{}, ", arg, arg).unwrap();
        }
        let raw_bytes = data.iter().fold(0, |sz, dt| match dt {
            Ok(s) => sz + s.len(),
            Err(_) => sz,
        });
        writeln!(&mut out,
                 r#") -> String {{
    let mut ret = Vec::with_capacity({});  // {}"#,
                 if html == "error.html" {
                     raw_bytes.next_power_of_two()
                 } else {
                     32 * 1024
                 },
                 raw_bytes)
            .unwrap();
        for dt in data {
            match dt {
                Ok(s) => writeln!(&mut out, "    ret.extend({:?}.as_bytes());", s).unwrap(),
                Err(i) => writeln!(&mut out, "    a{}.commit(&mut ret);", i).unwrap(),
            }
        }
        writeln!(&mut out, "    ret.extend({:?}.as_bytes());", &with_assets[idx..]).unwrap();

        writeln!(&mut out,
                 r#"
    ret.shrink_to_fit();
    unsafe {{ String::from_utf8_unchecked(ret) }}
}}"#)
            .unwrap();
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
