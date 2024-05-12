extern crate embed_resource;
extern crate base64;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern crate cc;

use std::{env, fs};
use std::path::Path;
use base64::display::Base64Display;



fn main() {
    assets();

    embed_resource::compile("http-manifest.rc");

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    cc::Build::new().file("build-ioctl.c").define("_GNU_SOURCE", "1").compile("http-ioctl");
}

fn assets() {
    let mut map = Vec::new();
    for (key, mime, file) in
        [("favicon", "image/x-icon", "assets/favicon.ico"),
         ("dir_icon", "image/gif", "assets/icons/directory.gif"),
         ("file_icon", "image/gif", "assets/icons/file.gif"),
         ("file_binary_icon", "image/gif", "assets/icons/file_binary.gif"),
         ("file_image_icon", "image/gif", "assets/icons/file_image.gif"),
         ("file_text_icon", "image/gif", "assets/icons/file_text.gif"),
         ("back_arrow_icon", "image/gif", "assets/icons/back_arrow.gif"),
         ("new_dir_icon", "image/gif", "assets/icons/new_directory.gif"),
         ("delete_file_icon", "image/png", "assets/icons/delete_file.png"),
         ("rename_icon", "image/png", "assets/icons/rename.png"),
         ("confirm_icon", "image/png", "assets/icons/confirm.png")] {
        println!("cargo::rerun-if-changed={}", file);
        map.push((key,
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
        println!("cargo::rerun-if-changed={}", file);
        map.push((key, fs::read_to_string(file).unwrap()));
    }

    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("assets.rs"),
              format!("static ASSETS: [(&'static str, &'static str); {}] = {:?};\n", map.len(), map))
        .unwrap();
}
