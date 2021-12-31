use iron::method;
use std::{fmt, str};
use xml::name::Name as XmlName;
use iron::url::Url as GenericUrl;
use iron::headers::{HeaderFormat, Header};
use iron::error::{HttpResult as HyperResult, HttpError as HyperError};


macro_rules! xml_name {
    ($ns:expr, $ln:expr) => {
        XmlName {
            local_name: $ln,
            namespace: Some($ns.1),
            prefix: Some($ns.0),
        }
    }
}


lazy_static! {
    /// HTTP methods we support for WebDAV level 1, as specified in https://tools.ietf.org/html/rfc2518, without locks
    pub static ref DAV_LEVEL_1_METHODS: Vec<method::Method> =
        ["COPY", "MKCOL", "MOVE", "PROPFIND", "PROPPATCH"].iter().map(|m| method::Extension(m.to_string())).collect();
}

/// Prefix and namespace URI for generic WebDAV elements
pub const WEBDAV_XML_NAMESPACE_DAV: (&str, &str) = ("D", "DAV:");

/// Prefix and namespace URI for elements specific to Windows clients
pub const WEBDAV_XML_NAMESPACE_MICROSOFT: (&str, &str) = ("Z", "urn:schemas-microsoft-com:");

/// Prefix and namespace URI for elements for Apache emulation
pub const WEBDAV_XML_NAMESPACE_APACHE: (&str, &str) = ("A", "http://apache.org/dav/props/");

/// All first-class-recognised prefix/namespace pairs
///
/// `WEBDAV_XML_NAMESPACE_DAV` needs to be the first here
pub const WEBDAV_XML_NAMESPACES: &[&(&str, &str)] = &[&WEBDAV_XML_NAMESPACE_DAV, &WEBDAV_XML_NAMESPACE_MICROSOFT, &WEBDAV_XML_NAMESPACE_APACHE];

/// Properties to return on empty body or [`<allprop />`](https://tools.ietf.org/html/rfc2518#section-12.14.1)
/// for non-Windows clients
///
/// Based on https://github.com/miquels/webdav-handler-rs/blob/02433c1acfccd848a7de26889f6857cbad559076/src/handle_props.rs#L52
pub const WEBDAV_ALLPROP_PROPERTIES_NON_WINDOWS: &[&[XmlName]] = &[&[xml_name!(WEBDAV_XML_NAMESPACE_DAV, "creationdate"),
                                                                     xml_name!(WEBDAV_XML_NAMESPACE_DAV, "getcontentlength"),
                                                                     xml_name!(WEBDAV_XML_NAMESPACE_DAV, "getcontenttype"),
                                                                     xml_name!(WEBDAV_XML_NAMESPACE_DAV, "getlastmodified"),
                                                                     xml_name!(WEBDAV_XML_NAMESPACE_DAV, "resourcetype")]];

/// Properties to return on empty body or [`<allprop />`](https://tools.ietf.org/html/rfc2518#section-12.14.1)
/// for Windows clients
///
/// Based on https://github.com/miquels/webdav-handler-rs/blob/02433c1acfccd848a7de26889f6857cbad559076/src/handle_props.rs#L66
pub const WEBDAV_ALLPROP_PROPERTIES_WINDOWS: &[&[XmlName]] = &[&WEBDAV_ALLPROP_PROPERTIES_NON_WINDOWS[0],
                                                               &[xml_name!(WEBDAV_XML_NAMESPACE_MICROSOFT, "Win32CreationTime"),
                                                                 xml_name!(WEBDAV_XML_NAMESPACE_MICROSOFT, "Win32FileAttributes"),
                                                                 xml_name!(WEBDAV_XML_NAMESPACE_MICROSOFT, "Win32LastAccessTime"),
                                                                 xml_name!(WEBDAV_XML_NAMESPACE_MICROSOFT, "Win32LastModifiedTime")]];

/// Properties listed for a [`<propname />`](https://tools.ietf.org/html/rfc2518#section-12.14.2) request
///
/// Based on https://github.com/miquels/webdav-handler-rs/blob/02433c1acfccd848a7de26889f6857cbad559076/src/handle_props.rs#L34
pub const WEBDAV_PROPNAME_PROPERTIES: &[&[XmlName]] = &[&WEBDAV_ALLPROP_PROPERTIES_NON_WINDOWS[0],
                                                        &[xml_name!(WEBDAV_XML_NAMESPACE_APACHE, "executable"),
                                                          xml_name!(WEBDAV_XML_NAMESPACE_MICROSOFT, "Win32LastAccessTime")]];



/// The [DAV header](https://tools.ietf.org/html/rfc2518#section-9.1), without parsing.
///
/// We don't ever receive this header, only ever send it, so this is fine.
#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct Dav(pub &'static [&'static str]);

impl Dav {
    pub const LEVEL_1: Dav = Dav(&["1"]);
}

impl Header for Dav {
    fn header_name() -> &'static str {
        "DAV"
    }

    /// Dummy impl returning an empty value, since we're only ever sending these
    fn parse_header(_: &[Vec<u8>]) -> HyperResult<Dav> {
        Ok(Dav(&[]))
    }
}

impl HeaderFormat for Dav {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0[0])?;
        for lvl in self.0.iter().skip(1) {
            f.write_str(", ")?;
            f.write_str(lvl)?;
        }
        Ok(())
    }
}

/// The [Depth header](https://tools.ietf.org/html/rfc2518#section-9.2).
#[derive(Debug, Copy, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub enum Depth {
    Zero,
    One,
    Infinity,
}

impl Depth {
    /// Get a depth lower than this one by one, if it exists
    pub fn lower(self) -> Option<Depth> {
        match self {
            Depth::Zero => None,
            Depth::One => Some(Depth::Zero),
            Depth::Infinity => Some(Depth::Infinity),
        }
    }
}

impl Header for Depth {
    fn header_name() -> &'static str {
        "Depth"
    }

    fn parse_header(raw: &[Vec<u8>]) -> HyperResult<Depth> {
        if raw.len() != 1 {
            return Err(HyperError::Header);
        }

        Ok(match &unsafe { raw.get_unchecked(0) }[..] {
            b"0" => Depth::Zero,
            b"1" => Depth::One,
            b"infinity" => Depth::Infinity,
            _ => return Err(HyperError::Header),
        })
    }
}

impl HeaderFormat for Depth {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Depth::Zero => f.write_str("0"),
            Depth::One => f.write_str("1"),
            Depth::Infinity => f.write_str("infinity"),
        }
    }
}

impl fmt::Display for Depth {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_header(f)
    }
}

/// The [Destination header](https://tools.ietf.org/html/rfc2518#section-9.3).
#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct Destination(pub GenericUrl);

impl Header for Destination {
    fn header_name() -> &'static str {
        "Destination"
    }

    fn parse_header(raw: &[Vec<u8>]) -> HyperResult<Destination> {
        if raw.len() != 1 {
            return Err(HyperError::Header);
        }

        let url = str::from_utf8(&unsafe { raw.get_unchecked(0) }).map_err(|_| HyperError::Header)?;
        GenericUrl::parse(url).map(Destination).map_err(HyperError::Uri)
    }
}

impl HeaderFormat for Destination {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl fmt::Display for Destination {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_header(f)
    }
}

/// The [Overwrite header](https://tools.ietf.org/html/rfc2518#section-9.6).
#[derive(Debug, Copy, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct Overwrite(pub bool);

impl Header for Overwrite {
    fn header_name() -> &'static str {
        "Overwrite"
    }

    fn parse_header(raw: &[Vec<u8>]) -> HyperResult<Overwrite> {
        if raw.len() != 1 {
            return Err(HyperError::Header);
        }

        let val = unsafe { raw.get_unchecked(0) };
        if val.len() != 1 {
            return Err(HyperError::Header);
        }
        match unsafe { val.get_unchecked(0) } {
            b'T' => Ok(Overwrite(true)),
            b'F' => Ok(Overwrite(false)),
            _ => Err(HyperError::Header),
        }
    }
}

impl HeaderFormat for Overwrite {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(if self.0 { "T" } else { "F" })
    }
}

impl fmt::Display for Overwrite {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_header(f)
    }
}

impl Default for Overwrite {
    #[inline(always)]
    fn default() -> Overwrite {
        Overwrite(true)
    }
}
