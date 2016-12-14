use std::path::PathBuf;
use self::super::{Options, Error};
use iron::{status, IronResult, Listening, Response, Request, Handler, Iron};


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
    fn handle(&self, _: &mut Request) -> IronResult<Response> {
        Ok(Response::with((status::Ok, format!("The abolishment of the burgeoisie.\n{:#?}\n", self.hosted_directory))))
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
