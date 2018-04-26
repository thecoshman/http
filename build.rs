extern crate embed_resource;
#[cfg(all(not(target_os = "windows"), not(target_os = "osx")))]
extern crate gcc;

#[cfg(all(not(target_os = "windows"), not(target_os = "osx")))]
use std::env;
#[cfg(all(not(target_os = "windows"), not(target_os = "osx")))]
use std::io::Write;
#[cfg(all(not(target_os = "windows"), not(target_os = "osx")))]
use std::path::Path;
#[cfg(all(not(target_os = "windows"), not(target_os = "osx")))]
use std::fs::{self, File};


/// The last line of this, after running it through a preprocessor, will expand to the value of `BLKGETSIZE`
#[cfg(all(not(target_os = "windows"), not(target_os = "osx")))]
static IOCTL_CHECK_SOURCE: &str = r#"
#include <linux/fs.h>

BLKGETSIZE
"#;

/// Replace `{}` with the `BLKGETSIZE` expression from `IOCTL_CHECK_SOURCE`
#[cfg(all(not(target_os = "windows"), not(target_os = "osx")))]
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

#[cfg(any(target_os = "windows", target_os = "osx"))]
fn get_ioctl_data() {}

#[cfg(all(not(target_os = "windows"), not(target_os = "osx")))]
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
