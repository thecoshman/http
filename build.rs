extern crate embed_resource;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern crate cc;


fn main() {
    embed_resource::compile("http-manifest.rc");

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    cc::Build::new().file("build-ioctl.c").define("_GNU_SOURCE", "1").compile("http-ioctl");
}
