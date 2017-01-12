#[macro_use]
extern crate lazy_static;
extern crate mime_guess;
#[cfg(feature = "clipboard")]
extern crate clipboard;
extern crate lazysort;
extern crate brotli2;
extern crate base64;
extern crate flate2;
extern crate bzip2;
extern crate hyper;
#[macro_use]
extern crate clap;
extern crate iron;
extern crate time;
extern crate url;
extern crate md6;

mod error;
mod options;

pub mod ops;
pub mod util;

pub use error::Error;
pub use options::Options;

use iron::Iron;
use std::io::stderr;
use std::process::exit;
#[cfg(feature = "clipboard")]
use clipboard::ClipboardContext;


fn main() {
    let result = actual_main();
    exit(result);
}

fn actual_main() -> i32 {
    if let Err(err) = result_main() {
        err.print_error(&mut stderr());
        err.exit_value()
    } else {
        0
    }
}

fn result_main() -> Result<(), Error> {
    let opts = Options::parse();

    let responder = try!(if let Some(p) = opts.port {
        Iron::new(ops::HttpHandler::new(&opts))
            .http(("0.0.0.0", p))
            .map_err(|_| {
                Error::Io {
                    desc: "server",
                    op: "start",
                    more: Some("port taken"),
                }
            })
    } else {
        ops::try_ports(ops::HttpHandler::new(&opts), util::PORT_SCAN_LOWEST, util::PORT_SCAN_HIGHEST)
    });

    println!("Hosting \"{}\" on port {}...", opts.hosted_directory.0, responder.socket.port());
    #[cfg(feature = "clipboard")]
    {
        if let Some(self_ip) = util::response_body("https://api.ipify.org") {
            if let Ok(mut clpbrd) = ClipboardContext::new() {
                if clpbrd.set_contents(format!("{}:{}", self_ip, responder.socket.port())).is_ok() {
                    println!("Externally-accessible URL is in the clipboard.");
                }
            }
        }
    }
    println!("Ctrl-C to stop.");
    println!();

    Ok(())
}
