extern crate hyper_native_tls;
extern crate percent_encoding;
extern crate trivial_colours;
#[cfg(not(target_os = "windows"))]
extern crate os_str_generic;
#[macro_use]
extern crate lazy_static;
extern crate serde_json;
extern crate mime_guess;
extern crate itertools;
extern crate tabwriter;
extern crate lazysort;
extern crate brotli2;
extern crate unicase;
extern crate walkdir;
extern crate base64;
extern crate flate2;
extern crate rfsapi;
#[cfg(target_os = "windows")]
extern crate winapi;
extern crate bzip2;
extern crate ctrlc;
extern crate serde;
extern crate regex;
extern crate cidr;
#[macro_use]
extern crate clap;
extern crate iron;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern crate libc;
extern crate rand;
extern crate time;
extern crate md6;
extern crate xml;

mod error;
mod options;

pub mod ops;
pub mod util;

pub use error::Error;
pub use options::{LogLevel, Options};

use std::mem;
use iron::Iron;
use std::net::IpAddr;
use std::process::exit;
use tabwriter::TabWriter;
use std::io::{Write, stdout};
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
    if opts.generate_tls {
        opts.tls_data = Some(ops::generate_tls_data(&opts.temp_directory)?);
    }
    for path in mem::replace(&mut opts.generate_path_auth, BTreeSet::new()) {
        opts.path_auth_data.insert(path, Some(ops::generate_auth_data()));
    }

    let mut responder = if let Some(p) = opts.port {
        if let Some(&((ref id, _), ref pw)) = opts.tls_data.as_ref() {
                Iron::new(ops::HttpHandler::new(&opts)).https((opts.bind_address, p),
                                                              NativeTlsServer::new(id, pw).map_err(|err| {
                        Error {
                            desc: "TLS certificate",
                            op: "open",
                            more: err.to_string().into(),
                        }
                    })?)
            } else {
                Iron::new(ops::HttpHandler::new(&opts)).http((opts.bind_address, p))
            }
            .map_err(|_| {
                Error {
                    desc: "server",
                    op: "start",
                    more: "port taken".into(),
                }
            })
    } else {
        ops::try_ports(ops::HttpHandler::new(&opts),
                       opts.bind_address,
                       util::PORT_SCAN_LOWEST,
                       util::PORT_SCAN_HIGHEST,
                       &opts.tls_data)
    }?;

    if opts.loglevel < options::LogLevel::NoStartup {
        print!("{}Hosting \"{}\" on port {}",
               trivial_colours::Reset,
               opts.hosted_directory.0,
               responder.socket.port());
        if responder.socket.ip() != IpAddr::from([0, 0, 0, 0]) {
            print!(" under address {}", responder.socket.ip());
        }
        print!(" with");
        if let Some(&((ref id, _), _)) = opts.tls_data.as_ref() {
            print!(" TLS certificate from \"{}\"", id);
        } else {
            print!("out TLS");
        }
        if !opts.path_auth_data.is_empty() {
            print!(" and basic authentication");
        } else {
            print!(" and no authentication");
        }
        println!("...");

        if !opts.proxies.is_empty()  {
            println!("Trusted proxies:");

            let mut out = TabWriter::new(stdout());
            writeln!(out, "Header\tNetwork").unwrap();
            for (network, header) in &opts.proxies {
                writeln!(out, "{}\t{}", header, network).unwrap();
            }
            out.flush().unwrap();
        }
    }
    if !opts.path_auth_data.is_empty() && opts.loglevel < options::LogLevel::NoAuth {
        println!("Basic authentication credentials:");

        let mut out = TabWriter::new(stdout());
        writeln!(out, "Path\tUsername\tPassword").unwrap();

        for (path, creds) in &opts.path_auth_data {
            if let Some(ad) = creds {
                let mut itr = ad.split(':');
                write!(out, "/{}\t{}\t", path, itr.next().unwrap()).unwrap();
                if let Some(p) = itr.next() {
                    write!(out, "{}", p).unwrap();
                }
                writeln!(out).unwrap();
            } else {
                writeln!(out, "/{}\t\t", path).unwrap();
            }
        }

        out.flush().unwrap();
    }
    if opts.loglevel < options::LogLevel::NoStartup {
        println!("Ctrl-C to stop.");
        println!();
    }

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
    ops::HttpHandler::clean_temp_dirs(&opts.temp_directory, opts.loglevel);

    Ok(())
}
