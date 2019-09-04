use iron::{status, IronResult, Response, Request};
use self::super::HttpHandler;


impl HttpHandler {
    pub(super) fn handle_webdav_copy(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log, "{:#?}", req);
        Ok(Response::with((status::MethodNotAllowed, "COPY unimplemented")))
    }

    pub(super) fn handle_webdav_lock(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log, "{:#?}", req);
        Ok(Response::with((status::MethodNotAllowed, "LOCK unimplemented")))
    }

    pub(super) fn handle_webdav_mkcol(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log, "{:#?}", req);
        Ok(Response::with((status::MethodNotAllowed, "MKCOL unimplemented")))
    }

    pub(super) fn handle_webdav_move(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log, "{:#?}", req);
        Ok(Response::with((status::MethodNotAllowed, "MOVE unimplemented")))
    }

    pub(super) fn handle_webdav_propfind(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log, "{:#?}", req);
        Ok(Response::with((status::MethodNotAllowed, "PROPFIND unimplemented")))
    }

    pub(super) fn handle_webdav_proppatch(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log, "{:#?}", req);
        Ok(Response::with((status::MethodNotAllowed, "PROPPATCH unimplemented")))
    }

    pub(super) fn handle_webdav_unlock(&self, req: &mut Request) -> IronResult<Response> {
        log!(self.log, "{:#?}", req);
        Ok(Response::with((status::MethodNotAllowed, "UNLOCK unimplemented")))
    }
}
