extern crate hyper_native_tls;
extern crate percent_encoding;
extern crate trivial_colours;
#[macro_use]
extern crate lazy_static;
extern crate serde_json;
extern crate mime_guess;
extern crate lazysort;
extern crate brotli2;
extern crate unicase;
extern crate base64;
extern crate flate2;
extern crate rfsapi;
extern crate bzip2;
extern crate ctrlc;
extern crate serde;
extern crate regex;
#[macro_use]
extern crate clap;
extern crate iron;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern crate libc;
extern crate rand;
extern crate time;
extern crate md6;

mod error;
mod options;

pub mod ops;
pub mod util;

pub use error::Error;
pub use options::Options;

use std::mem;
use iron::Iron;
use std::process::exit;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, Condvar};
use hyper_native_tls::NativeTlsServer;


fn main() {
    let result = actual_main();
    exit(result);
}

fn actual_main() -> i32 {
    if let Err(err) = result_main() {
        eprintln!("{}", err);
        1
    } else {
        0
    }
}

fn result_main() -> Result<(), Error> {
    let mut opts = Options::parse();
    println!("{:#?}", opts);
    if opts.generate_tls {
        opts.tls_data = Some(try!(ops::generate_tls_data(&opts.temp_directory)));
    }
    if opts.generate_global_auth {
        opts.global_auth_data = Some(ops::generate_auth_data());
    }
    for path in mem::replace(&mut opts.generate_path_auth, BTreeSet::new()) {
        opts.path_auth_data.insert(path, Some(ops::generate_auth_data()));
    }
    println!("{:#?}", opts);

    let mut responder = try!(if let Some(p) = opts.port {
        if let Some(&((ref id, _), ref pw)) = opts.tls_data.as_ref() {
                Iron::new(ops::HttpHandler::new(&opts)).https(("0.0.0.0", p),
                                                              try!(NativeTlsServer::new(id, pw).map_err(|err| {
                    Error {
                        desc: "TLS certificate",
                        op: "open",
                        more: err.to_string().into(),
                    }
                })))
            } else {
                Iron::new(ops::HttpHandler::new(&opts)).http(("0.0.0.0", p))
            }
            .map_err(|_| {
                Error {
                    desc: "server",
                    op: "start",
                    more: "port taken".into(),
                }
            })
    } else {
        ops::try_ports(ops::HttpHandler::new(&opts), util::PORT_SCAN_LOWEST, util::PORT_SCAN_HIGHEST, &opts.tls_data)
    });

    print!("{}Hosting \"{}\" on port {} with",
           trivial_colours::Reset,
           opts.hosted_directory.0,
           responder.socket.port());
    if let Some(&((ref id, _), _)) = opts.tls_data.as_ref() {
        print!(" TLS certificate from \"{}\"", id);
    } else {
        print!("out TLS");
    }
    print!(" and ");
    if let Some(ad) = opts.global_auth_data.as_ref() {
        let mut itr = ad.split(':');
        print!("basic global authentication using \"{}\" as username and ", itr.next().unwrap());
        if let Some(p) = itr.next() {
            print!("\"{}\" as", p);
        } else {
            print!("no");
        }
        print!(" password");
    } else {
        print!("no global authentication");
    }
    for (path, creds) in &opts.path_auth_data {
        print!(", ");
        if let Some(ad) = creds {
            let mut itr = ad.split(':');
            print!("basic authentication under \"{}\" using \"{}\" as username and ", path, itr.next().unwrap());
            if let Some(p) = itr.next() {
                print!("\"{}\" as", p);
            } else {
                print!("no");
            }
            print!(" password");
        } else {
            print!("no authentication under \"{}\"", path);
        }
    }
    println!("...");
    println!("Ctrl-C to stop.");
    println!();

    let end_handler = Arc::new(Condvar::new());
    ctrlc::set_handler({
            let r = end_handler.clone();
            move || r.notify_one()
        })
        .unwrap();
    let mx = Mutex::new(());
    let _ = end_handler.wait(mx.lock().unwrap()).unwrap();
    responder.close().unwrap();

    // This is necessary because the server isn't Drop::drop()ped when the responder is
    ops::HttpHandler::clean_temp_dirs(&opts.temp_directory);

    Ok(())
}
