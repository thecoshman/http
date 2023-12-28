extern crate embed_resource;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern crate cc;


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
    cc::Build::new().file("build-ioctl.c").define("_GNU_SOURCE", "1").compile("http-ioctl");
}
