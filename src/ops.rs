use std::path::PathBuf;
use iron::modifiers::Header;
use self::super::{Options, Error};
use mime_guess::guess_mime_type_opt;
use self::super::util::{html_response, file_contains, ERROR_HTML, DIRECTORY_LISTING_HTML};
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
                    Ok(Response::with((status::NotFound,
                                       "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                                       html_response(ERROR_HTML,
                                                     vec!["404 Not Found".to_string(),
                                                          format!("The requested entity \"{}\" doesn't exist.",
                                                                  &req.url.path().into_iter().fold("".to_string(), |cur, pp| cur + "/" + pp)[1..]),
                                                          "".to_string()]))))
                } else if req_p.is_file() {
                    let mime_type = guess_mime_type_opt(&req_p).unwrap_or_else(|| if file_contains(&req_p, 0) {
                        "application/octet-stream".parse().unwrap()
                    } else {
                        "text/plain".parse().unwrap()
                    });
                    println!("{} was served file {} as {}", req.remote_addr, req_p.display(), mime_type);
                    Ok(Response::with((status::Ok, mime_type, req_p)))
                } else {
                    let relpath = (req.url.path().into_iter().fold("".to_string(), |cur, pp| cur + "/" + pp)[1..].to_string() + "/").replace("//", "/");
                    println!("{} was served directory listing for {}", req.remote_addr, req_p.display());
                    Ok(Response::with((status::Ok,
                                       "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                                       html_response(DIRECTORY_LISTING_HTML,
                                                     vec![relpath.clone(),
                                                          req_p.read_dir().unwrap().map(Result::unwrap).fold("".to_string(), |cur, f| {
                        let fname = f.file_name().into_string().unwrap() +
                                    if !f.file_type().unwrap().is_file() {
                            "/"
                        } else {
                            ""
                        };
                        cur + "<li><a href=\"" + &format!("/{}", relpath).replace("//", "/") + &fname + "\">" + &fname + "</a></li>\n"
                    })]))))
                }
            }
            ref m => {
                println!("{} used invalid request method {}", req.remote_addr, m);
                Ok(Response::with((status::NotImplemented,
                                   "text/html;charset=utf-8".parse::<mime::Mime>().unwrap(),
                                   html_response(ERROR_HTML,
                                                 vec!["501 Not Implemented".to_string(),
                                                      "This operation was not implemented.".to_string(),
                                                      format!("<p>Unsupported request method: {}.<br />Supported methods: OPTIONS and GET.</p>", m)]))))
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
