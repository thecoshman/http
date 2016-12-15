use std::path::PathBuf;
use iron::modifiers::Header;
use self::super::{Options, Error};
use self::super::util::{NOT_FOUND_HTML, NOT_IMPLEMENTED_HTML};
use iron::{headers, status, method, mime, IronResult, Listening, Response, Request, Handler, Iron};


#[derive(Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct HttpHandler {
    pub hosted_directory: (String, PathBuf),
}

impl HttpHandler {
    pub fn new(opts: &Options) -> HttpHandler {
        HttpHandler { hosted_directory: opts.hosted_directory.clone() }
    }
}

impl Handler for HttpHandler {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        match req.method {
            method::Options => Ok(Response::with((status::Ok, Header(headers::Allow(vec![method::Options, method::Get]))))),
            method::Get => {
                let req_p = req.url.path().into_iter().filter(|p| !p.is_empty()).fold(self.hosted_directory.1.clone(), |cur, pp| cur.join(pp));
                if !req_p.exists() {
                    println!("{} requested nonexistant file {}", req.remote_addr, req_p.display());
                    Ok(Response::with((status::NotFound, mime::Mime(mime::TopLevel::Text, mime::SubLevel::Html, vec![]), NOT_FOUND_HTML)))
                } else if req_p.is_file() {
                    println!("{} was served file {}", req.remote_addr, req_p.display());
                    Ok(Response::with((status::Ok, req_p)))
                } else {
                    println!("{} was served directory listing {}", req.remote_addr, req_p.display());
                    Ok(Response::with((status::Ok,
                                       format!("Contents of {}:\n{}",
                                               req.url.path().into_iter().fold(self.hosted_directory.0.clone(), |cur, pp| cur + "/" + pp),
                                               req_p.read_dir().unwrap().map(Result::unwrap).fold("".to_string(), |cur, f| {
                        cur + "  * " + &f.file_name().into_string().unwrap() +
                        if f.file_type().unwrap().is_dir() {
                            "/"
                        } else {
                            ""
                        } + "\n"
                    })))))
                }
            }
            ref m => {
                println!("{} used invalid request method {}", req.remote_addr, m);
                Ok(Response::with((status::NotImplemented, mime::Mime(mime::TopLevel::Text, mime::SubLevel::Html, vec![]), NOT_IMPLEMENTED_HTML)))
            }
        }
    }
}


pub fn try_ports<H: Handler + Clone>(hndlr: H, from: u16, up_to: u16) -> Result<Listening, Error> {
    for port in from..up_to {
        match Iron::new(hndlr.clone()).http(("0.0.0.0", port)) {
            Ok(server) => return Ok(server),
            Err(error) => {
                if !error.to_string().contains("port") {
                    return Err(Error::Io {
                        desc: "server",
                        op: "start",
                        more: None,
                    });
                }
            }
        }
    }

    Err(Error::Io {
        desc: "server",
        op: "start",
        more: Some("no free ports"),
    })
}
