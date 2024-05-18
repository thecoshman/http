use method::Method;

/// `Allow` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.4.1)
///
/// The `Allow` header field lists the set of methods advertised as
/// supported by the target resource.  The purpose of this field is
/// strictly to inform the recipient of valid request methods associated
/// with the resource.
///
/// # ABNF
/// ```plain
/// Allow = #method
/// ```
///
/// # Example values
/// * `GET, HEAD, PUT`
/// * `OPTIONS, GET, PUT, POST, DELETE, HEAD, TRACE, CONNECT, PATCH, fOObAr`
/// * ``
///
/// # Examples
/// ```
/// use hyper::header::{Headers, Allow};
/// use hyper::method::Method;
///
/// let mut headers = Headers::new();
/// headers.set(
///     Allow(vec![Method::Get])
/// );
/// ```
/// ```
/// use hyper::header::{Headers, Allow};
/// use hyper::method::Method;
///
/// let mut headers = Headers::new();
/// headers.set(
///     Allow(vec![
///         Method::Get,
///         Method::Post,
///         Method::Patch,
///         Method::Extension("TEST".to_owned()),
///     ].into())
/// );
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Allow(pub std::borrow::Cow<'static, [Method]>);
__hyper__deref!(Allow => std::borrow::Cow<'static, [Method]>);
impl ::header::Header for Allow {
    fn header_name() -> &'static str {
        "Allow"
    }
    fn parse_header<T: AsRef<[u8]>>(raw: &[T]) -> ::Result<Self> {
        ::header::parsing::from_comma_delimited(raw).map(std::borrow::Cow::Owned).map(Allow)
    }
}
impl ::header::HeaderFormat for Allow {
    fn fmt_header(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::header::parsing::fmt_comma_delimited(f, &self.0[..])
    }
}
impl ::std::fmt::Display for Allow {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        use ::header::HeaderFormat;
        self.fmt_header(f)
    }
}

bench_header!(bench,
Allow, { vec![b"OPTIONS,GET,PUT,POST,DELETE,HEAD,TRACE,CONNECT,PATCH,fOObAr".to_vec()] });
