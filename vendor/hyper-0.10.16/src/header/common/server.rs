/// `Server` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.4.2)
///
/// The `Server` header field contains information about the software
/// used by the origin server to handle the request, which is often used
/// by clients to help identify the scope of reported interoperability
/// problems, to work around or tailor requests to avoid particular
/// server limitations, and for analytics regarding server or operating
/// system use.  An origin server MAY generate a Server field in its
/// responses.
///
/// # ABNF
/// ```plain
/// Server = product *( RWS ( product / comment ) )
/// ```
///
/// # Example values
/// * `CERN/3.0 libwww/2.17`
///
/// # Example
/// ```
/// use hyper::header::{Headers, Server};
///
/// let mut headers = Headers::new();
/// headers.set(Server("hyper/0.5.2".to_owned()));
/// ```
// TODO: Maybe parse as defined in the spec?
#[derive(Clone, Debug, PartialEq)]
pub struct Server(pub std::borrow::Cow<'static, str>);
impl ::header::Header for Server {
    fn header_name() -> &'static str {
        "Server"
    }
    fn parse_header<T: AsRef<[u8]>>(raw: &[T]) -> ::Result<Self> {
        ::header::parsing::from_one_raw_str(raw).map(std::borrow::Cow::Owned).map(Server)
    }
}
impl ::header::HeaderFormat for Server {
    fn fmt_header(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::std::fmt::Display::fmt(&**self, f)
    }
}
impl ::std::fmt::Display for Server {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::std::fmt::Display::fmt(&**self, f)
    }
}
impl ::std::ops::Deref for Server {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

bench_header!(bench, Server, { vec![b"Some String".to_vec()] });
