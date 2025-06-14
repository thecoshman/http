#![cfg_attr(target_os = "windows", feature(windows_by_handle))]
#![allow(named_arguments_used_positionally)]

extern crate hyper_native_tls;
extern crate percent_encoding;
extern crate trivial_colours;
extern crate serde_json;
extern crate mime_guess;
extern crate tabwriter;
extern crate arrayvec;
extern crate walkdir;
extern crate blake3;
extern crate brotli;
extern crate flate2;
extern crate rfsapi;
#[cfg(target_os = "windows")]
extern crate winapi;
extern crate ctrlc;
extern crate serde;
extern crate cidr;
#[macro_use]
extern crate clap;
extern crate iron;
extern crate libc;
extern crate time;
extern crate xml;

mod options;

pub mod ops;
pub mod util;

pub struct Error(pub String);
pub use options::{LogLevel, Options};

use std::mem;
use libc::exit;
use iron::Iron;
use std::net::IpAddr;
use std::time::Duration;
use tabwriter::TabWriter;
use std::io::{Write, stdout};
use std::collections::BTreeSet;
use std::sync::{Mutex, Condvar};
use hyper_native_tls::NativeTlsServer;


fn main() {
    let result = actual_main();
    unsafe { exit(result) }
}

fn actual_main() -> i32 {
    if let Err(err) = result_main() {
        eprintln!("{}", err.0);
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

    let handler: &_ = Box::leak(Box::new(ops::SimpleChain::<ops::PruneChain, _> {
        handler: ops::PruneChain::new(&opts),
        after: opts.request_bandwidth.map(ops::LimitBandwidthMiddleware::new),
    }));
    let mut responder = if let Some(p) = opts.port {
        if let Some(&((_, ref id), ref pw)) = opts.tls_data.as_ref() {
                Iron::new(handler).https((opts.bind_address, p),
                                         NativeTlsServer::new(id, pw).map_err(|err| Error(format!("Opening TLS certificate: {}", err)))?)
            } else {
                Iron::new(handler).http((opts.bind_address, p))
            }
            .map_err(|_| Error(format!("Starting server: port taken")))
    } else {
        ops::try_ports(handler, opts.bind_address, util::PORT_SCAN_LOWEST, util::PORT_SCAN_HIGHEST, &opts.tls_data)
    }?;

    if opts.loglevel < options::LogLevel::NoStartup {
        if opts.log_colour {
            print!("{}", trivial_colours::Reset);
        }
        print!("Hosting \"{}\" on port {}", opts.hosted_directory.0, responder.socket.port());
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

        if let Some(band) = opts.request_bandwidth {
            println!("Requests limited to {}B/s.", band);
        }

        for (ext, mime_type) in opts.mime_type_overrides {
            match &ext.to_string_lossy()[..] {
                "" => println!("Serving files with no extension as {}.", mime_type),
                ext => println!("Serving files with .{} extension as {}.", ext, mime_type),
            }
        }

        if !opts.proxies.is_empty() {
            println!("Trusted proxies:");

            let mut out = TabWriter::new(stdout());
            writeln!(out, "Header\tNetwork").unwrap();
            for (network, header) in &opts.proxies {
                writeln!(out, "{}\t{}", header, network).unwrap();
            }
            writeln!(out, "URL Header\tNetwork").unwrap();
            for (network, header) in &opts.proxy_redirs {
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
    let Options { encoded_prune: opts_encoded_prune, temp_directory: opts_temp_directory, generate_tls: opts_generate_tls, .. } = opts;

    static END_HANDLER: Condvar = Condvar::new();
    ctrlc::set_handler(|| END_HANDLER.notify_one()).unwrap();
    if opts_encoded_prune.is_some() {
        loop {
            if !END_HANDLER.wait_timeout(Mutex::new(()).lock().unwrap(), Duration::from_secs(handler.handler.prune_interval)).unwrap().1.timed_out() {
                break;
            }

            handler.handler.prune();
        }
    } else {
        drop(END_HANDLER.wait(Mutex::new(()).lock().unwrap()).unwrap());
    }

    responder.close().unwrap();
    handler.handler.handler.clean_temp_dirs(&opts_temp_directory, opts_generate_tls);
    Ok(())
}
