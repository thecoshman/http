use iron::Handler;
use std::path::PathBuf;
use self::super::Options;
use iron::{status, IronResult, Response, Request};


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
