#[macro_use]
extern crate lazy_static;
extern crate mime_guess;
extern crate lazysort;
extern crate base64;
#[macro_use]
extern crate clap;
extern crate iron;
extern crate time;
extern crate url;

mod error;
mod options;

pub mod ops;
pub mod util;

pub use error::Error;
pub use options::Options;

use iron::Iron;
use std::io::stderr;
use std::process::exit;


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
    println!("Ctrl-C to stop.");
    println!();

    Ok(())
}
