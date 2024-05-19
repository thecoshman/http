//! Server Responses
//!
//! These are responses sent by a `hyper::Server` to clients, after
//! receiving a request.
use std::any::{Any, TypeId};
use std::marker::PhantomData;
use std::mem;
use std::io::{self, Write};
use std::ptr;
use std::thread;

use time::now_utc;

use header;
use http::h1::{LINE_ENDING, HttpWriter};
use http::h1::HttpWriter::{ThroughWriter, SizedWriter};
use status;
use net::{Fresh, Streaming};
use version;


/// The outgoing half for a Tcp connection, created by a `Server` and given to a `Handler`.
///
/// The default `StatusCode` for a `Response` is `200 OK`.
///
/// There is a `Drop` implementation for `Response` that will automatically
/// write the head and flush the body, if the handler has not already done so,
/// so that the server doesn't accidentally leave dangling requests.
#[derive(Debug)]
pub struct Response<'a, W: Any = Fresh> {
    /// The HTTP version of this response.
    pub version: version::HttpVersion,
    // Stream the Response is writing to, not accessible through UnwrittenResponse
    body: HttpWriter<&'a mut (Write + 'a)>,
    // The status code for the request.
    status: status::StatusCode,
    // The outgoing headers on this response.
    headers: &'a mut header::Headers,

    _writing: PhantomData<W>
}

impl<'a, W: Any> Response<'a, W> {
    /// The status of this response.
    #[inline]
    pub fn status(&self) -> status::StatusCode { self.status }

    /// The headers of this response.
    #[inline]
    pub fn headers(&self) -> &header::Headers { &*self.headers }

    /// Construct a Response from its constituent parts.
    #[inline]
    pub fn construct(version: version::HttpVersion,
                     body: HttpWriter<&'a mut (Write + 'a)>,
                     status: status::StatusCode,
                     headers: &'a mut header::Headers) -> Response<'a, Fresh> {
        Response {
            status: status,
            version: version,
            body: body,
            headers: headers,
            _writing: PhantomData,
        }
    }

    /// Deconstruct this Response into its constituent parts.
    #[inline]
    pub fn deconstruct(self) -> (version::HttpVersion, HttpWriter<&'a mut (Write + 'a)>,
                                 status::StatusCode, &'a mut header::Headers) {
        unsafe {
            let parts = (
                self.version,
                ptr::read(&self.body),
                self.status,
                ptr::read(&self.headers)
            );
            mem::forget(self);
            parts
        }
    }

    fn write_head(&mut self) -> io::Result<Body> {
        debug!("writing head: {:?} {:?}", self.version, self.status);
        try!(write!(&mut self.body, "{} {}\r\n", self.version, self.status));

        if !self.headers.has::<header::Date>() {
            self.headers.set(header::Date(header::HttpDate(now_utc())));
        }

        let body_type = match self.status {
            status::StatusCode::NoContent | status::StatusCode::NotModified => Body(0),
            c if c.class() == status::StatusClass::Informational => Body(0),
            _ => if let Some(cl) = self.headers.get::<header::ContentLength>() {
                Body(**cl)
            } else {
                panic!("Body::Chunked");
            }
        };

        debug!("headers [\n{:?}]", self.headers);
        try!(write!(&mut self.body, "{}{}", self.headers, LINE_ENDING));

        Ok(body_type)
    }
}

impl<'a> Response<'a, Fresh> {
    /// Creates a new Response that can be used to write to a network stream.
    #[inline]
    pub fn new(stream: &'a mut (Write + 'a), headers: &'a mut header::Headers) ->
            Response<'a, Fresh> {
        Response {
            status: status::StatusCode::Ok,
            version: version::HttpVersion::Http11,
            headers: headers,
            body: ThroughWriter(stream),
            _writing: PhantomData,
        }
    }

    /// Writes the body and ends the response.
    ///
    /// This is a shortcut method for when you have a response with a fixed
    /// size, and would only need a single `write` call normally.
    ///
    /// # Example
    ///
    /// ```
    /// # use hyper::server::Response;
    /// fn handler(res: Response) {
    ///     res.send(b"Hello World!").unwrap();
    /// }
    /// ```
    ///
    /// The above is the same, but shorter, than the longer:
    ///
    /// ```
    /// # use hyper::server::Response;
    /// use std::io::Write;
    /// use hyper::header::ContentLength;
    /// fn handler(mut res: Response) {
    ///     let body = b"Hello World!";
    ///     res.headers_mut().set(ContentLength(body.len() as u64));
    ///     let mut res = res.start().unwrap();
    ///     res.write_all(body).unwrap();
    /// }
    /// ```
    #[inline]
    pub fn send(self, body: &[u8]) -> io::Result<()> {
        self.headers.set(header::ContentLength(body.len() as u64));
        let mut stream = try!(self.start());
        try!(stream.writer().write_all(body));
        stream.end()
    }

    /// Consume this Response<Fresh>, writing the Headers and Status and
    /// creating a Response<Streaming>
    pub fn start(mut self) -> io::Result<Response<'a, Streaming>> {
        let body_type = try!(self.write_head());
        let (version, body, status, headers) = self.deconstruct();
        let stream = SizedWriter(body.into_inner(), body_type.0);

        // "copy" to change the phantom type
        Ok(Response {
            version: version,
            body: stream,
            status: status,
            headers: headers,
            _writing: PhantomData,
        })
    }
    /// Get a mutable reference to the status.
    #[inline]
    pub fn status_mut(&mut self) -> &mut status::StatusCode { &mut self.status }

    /// Get a mutable reference to the Headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut header::Headers { self.headers }
}


impl<'a> Response<'a, Streaming> {
    /// Flushes all writing of a response to the client.
    #[inline]
    pub fn end(self) -> io::Result<()> {
        trace!("ending");
        let (_, body, _, _) = self.deconstruct();
        try!(body.end());
        Ok(())
    }
}

impl<'a> Response<'a, Streaming> {
    pub fn writer(&mut self) -> &mut HttpWriter<&'a mut (Write + 'a)> {
        &mut self.body
    }
}

#[derive(PartialEq, Debug)]
struct Body(u64);

impl<'a, T: Any> Drop for Response<'a, T> {
    fn drop(&mut self) {
        if TypeId::of::<T>() == TypeId::of::<Fresh>() {
            if thread::panicking() {
                self.status = status::StatusCode::InternalServerError;
                if self.headers.get::<header::ContentLength>().is_none() {
                    self.headers.set(header::ContentLength(0));
                }
            }

            let mut body = match self.write_head() {
                Ok(Body(len)) => SizedWriter(self.body.get_mut(), len),
                Err(e) => {
                    debug!("error dropping request: {:?}", e);
                    return;
                }
            };
            end(&mut body);
        } else {
            end(&mut self.body);
        };


        #[inline]
        fn end<W: Write>(w: &mut W) {
            match w.write(&[]) {
                Ok(_) => match w.flush() {
                    Ok(_) => debug!("drop successful"),
                    Err(e) => debug!("error dropping request: {:?}", e)
                },
                Err(e) => debug!("error dropping request: {:?}", e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use header::Headers;
    use mock::MockStream;
    use super::Response;

    macro_rules! lines {
        ($s:ident = $($line:pat),+) => ({
            let s = String::from_utf8($s.write).unwrap();
            let mut lines = s.split_terminator("\r\n");

            $(
                match lines.next() {
                    Some($line) => (),
                    other => panic!("line mismatch: {:?} != {:?}", other, stringify!($line))
                }
            )+

            assert_eq!(lines.next(), None);
        })
    }

    #[test]
    fn test_fresh_start() {
        let mut headers = Headers::new();
        let mut stream = MockStream::new();
        {
            let res = Response::new(&mut stream, &mut headers);
            res.start().unwrap().deconstruct();
        }

        lines! { stream =
            "HTTP/1.1 200 OK",
            _date,
            _transfer_encoding,
            ""
        }
    }

    #[test]
    fn test_streaming_end() {
        let mut headers = Headers::new();
        let mut stream = MockStream::new();
        {
            let res = Response::new(&mut stream, &mut headers);
            res.start().unwrap().end().unwrap();
        }

        lines! { stream =
            "HTTP/1.1 200 OK",
            _date,
            _transfer_encoding,
            "",
            "0",
            "" // empty zero body
        }
    }

    #[test]
    fn test_fresh_drop() {
        use status::StatusCode;
        let mut headers = Headers::new();
        let mut stream = MockStream::new();
        {
            let mut res = Response::new(&mut stream, &mut headers);
            *res.status_mut() = StatusCode::NotFound;
        }

        lines! { stream =
            "HTTP/1.1 404 Not Found",
            _date,
            _transfer_encoding,
            "",
            "0",
            "" // empty zero body
        }
    }

    // x86 windows msvc does not support unwinding
    // See https://github.com/rust-lang/rust/issues/25869
    #[cfg(not(all(windows, target_arch="x86", target_env="msvc")))]
    #[test]
    fn test_fresh_drop_panicing() {
        use std::thread;
        use std::sync::{Arc, Mutex};

        use status::StatusCode;

        let stream = MockStream::new();
        let stream = Arc::new(Mutex::new(stream));
        let inner_stream = stream.clone();
        let join_handle = thread::spawn(move || {
            let mut headers = Headers::new();
            let mut stream = inner_stream.lock().unwrap();
            let mut res = Response::new(&mut *stream, &mut headers);
            *res.status_mut() = StatusCode::NotFound;

            panic!("inside")
        });

        assert!(join_handle.join().is_err());

        let stream = match stream.lock() {
            Err(poisoned) => poisoned.into_inner().clone(),
            Ok(_) => unreachable!()
        };

        lines! { stream =
            "HTTP/1.1 500 Internal Server Error",
            _date,
            _transfer_encoding,
            "",
            "0",
            "" // empty zero body
        }
    }


    #[test]
    fn test_streaming_drop() {
        use std::io::Write;
        use status::StatusCode;
        let mut headers = Headers::new();
        let mut stream = MockStream::new();
        {
            let mut res = Response::new(&mut stream, &mut headers);
            *res.status_mut() = StatusCode::NotFound;
            let mut stream = res.start().unwrap();
            stream.write_all(b"foo").unwrap();
        }

        lines! { stream =
            "HTTP/1.1 404 Not Found",
            _date,
            _transfer_encoding,
            "",
            "3",
            "foo",
            "0",
            "" // empty zero body
        }
    }

    #[test]
    fn test_no_content() {
        use status::StatusCode;
        let mut headers = Headers::new();
        let mut stream = MockStream::new();
        {
            let mut res = Response::new(&mut stream, &mut headers);
            *res.status_mut() = StatusCode::NoContent;
            res.start().unwrap();
        }

        lines! { stream =
            "HTTP/1.1 204 No Content",
            _date,
            ""
        }
    }
}
