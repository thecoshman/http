fn main() {
    eprintln!("");
    eprintln!("With http 2.0.0, http is no longer publishable on crates.io.");
    eprintln!("This is for a mundane reason of needing to vendor patched dependencies: https://github.com/thecoshman/http/pull/160#issuecomment-2143877822");
    eprintln!("");
    eprintln!("Please install http from git by running");
    #[cfg(not(windows))]
    eprintln!("  RUSTC_BOOTSTRAP=1 cargo install -f --git https://github.com/thecoshman/http");
    #[cfg(windows)]
    {
        eprintln!("  set RUSTC_BOOTSTRAP=1");
        eprintln!("  cargo install -f --git https://github.com/thecoshman/http");
    }
    eprintln!("and then update as normal.");
    eprintln!("For use with cargo-update, also do");
    eprintln!("  cargo install-update-config -e RUSTC_BOOTSTRAP=1 https");
    eprintln!("");
    eprintln!("You will continue to only receive normal, full, releases.");
    eprintln!("");
    std::process::exit(1);
}
