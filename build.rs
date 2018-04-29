extern crate embed_resource;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern crate gcc;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use std::env;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use std::io::Write;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use std::path::Path;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use std::fs::{self, File};


/// The last line of this, after running it through a preprocessor, will expand to the value of `BLKGETSIZE`
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
static IOCTL_CHECK_SOURCE: &str = r#"
#include <linux/fs.h>

BLKGETSIZE
"#;

/// Replace `{}` with the `BLKGETSIZE` expression from `IOCTL_CHECK_SOURCE`
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
static IOCTL_INCLUDE_SKELETON: &str = r#"
/// Return `device size / 512` (`long *` arg)
static BLKGETSIZE: c_ulong = {};
"#;


fn main() {
    embed_resources();
    get_ioctl_data();
}

fn embed_resources() {
    embed_resource::compile("http-manifest.rc");
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn get_ioctl_data() {}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn get_ioctl_data() {
    let ioctl_dir = Path::new(&env::var("OUT_DIR").unwrap()).join("ioctl-data");
    fs::create_dir_all(&ioctl_dir).unwrap();

    let ioctl_source = ioctl_dir.join("ioctl.c");
    File::create(&ioctl_source).unwrap().write_all(IOCTL_CHECK_SOURCE.as_bytes()).unwrap();

    let ioctl_preprocessed = String::from_utf8(gcc::Build::new().file(ioctl_source).expand()).unwrap();
    let blkgetsize_expr = ioctl_preprocessed.lines().next_back().unwrap().replace("U", " as c_ulong");

    let ioctl_include = ioctl_dir.join("ioctl.rs");
    File::create(&ioctl_include).unwrap().write_all(IOCTL_INCLUDE_SKELETON.replace("{}", &blkgetsize_expr).as_bytes()).unwrap();
}
