//! Raw Filesystem API for Rust — enable simpler browsing with ease
//!
//! This library is to enable both servers and clients to use the RFSAPI,
//! see [D'Oh](https://github.com/thecoshman/doh) for usage example.


#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate hyper;
extern crate time;
extern crate mime;

use time::Tm;
use mime::Mime;
use util::parse_rfc3339;
use std::fmt::{self, Write};
use hyper::Error as HyperError;
use serde::ser::{SerializeMap, Serializer, Serialize};
use hyper::header::{Formatter as HeaderFormatter, Raw as RawHeader, Header};
use serde::de::{self, Deserializer, Deserialize, SeqAccess, MapAccess, Visitor};

pub mod util;


static RAW_FILE_DATA_FIELDS: &[&str] = &["mime_type", "name", "last_modified", "size", "is_file"];


/// Header to specify when doing a request for the Raw Filesystem API,
/// designated by "X-Raw-Filesystem-API".
///
/// If RFSAPI is supported, the server should return the header set to true.
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash, Copy)]
pub struct RawFsApiHeader(pub bool);

impl Header for RawFsApiHeader {
    fn header_name() -> &'static str {
        "X-Raw-Filesystem-API"
    }

    fn parse_header(raw: &RawHeader) -> Result<RawFsApiHeader, HyperError> {
        if let Some(line) = raw.one() {
            match &line[..] {
                b"0" => return Ok(RawFsApiHeader(false)),
                b"1" => return Ok(RawFsApiHeader(true)),
                _ => {}
            }
        }
        Err(HyperError::Header)
    }

    fn fmt_header(&self, f: &mut HeaderFormatter) -> fmt::Result {
        f.fmt_line(&self)
    }
}

impl fmt::Display for RawFsApiHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char(if self.0 { '1' } else { '0' })
    }
}


/// Basic RFSAPI response returned by the server.
///
/// # Examples
///
/// ```
/// # use rfsapi::FilesetData;
/// # mod serde_json {
/// #     use rfsapi::FilesetData;
/// #     pub fn from_str(_: &str) -> FilesetData {
/// #         FilesetData { writes_supported: true, is_root: true,
/// #                       is_file: false, files: vec![] } } }
/// # let server_response = "";
/// let resp: FilesetData = serde_json::from_str(server_response);
/// println!("Requested directory has {} children.", resp.files.len());
/// ```
#[derive(Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct FilesetData {
    /// Whether PUT and DELETE requests are allowed on the server.
    pub writes_supported: bool,
    /// Whether the requested directory is the root (topmost).
    ///
    /// `false` if a singular file was requested.
    pub is_root: bool,
    /// Whether the requested resource is a file.
    pub is_file: bool,
    /// List of requested files.
    ///
    /// If the requested resource is a directory, its immediate children are
    /// returned here.
    ///
    /// If the requested resource is a file, its information is returned as the
    /// only element.
    pub files: Vec<RawFileData>,
}

/// Information about a file available through RFSAPI.
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct RawFileData {
    /// File's determined MIME type.
    ///
    /// Always valid, but possibly garbage for directories.
    /// Recommended value for directories: `"text/directory"`.
    pub mime_type: Mime,
    /// File's name, which can be used to navigate to it.
    pub name: String,
    /// File's last modification time, as returned by the FS.
    pub last_modified: Tm,
    /// File size in bytes.
    ///
    /// Possibly garbage for directories.
    /// Recommended value for directories: `0`.
    pub size: u64,
    /// Whether the file is a file.
    pub is_file: bool,
}

impl Serialize for RawFileData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = try!(serializer.serialize_map(Some(RAW_FILE_DATA_FIELDS.len())));
        try!(map.serialize_entry("mime_type", &self.mime_type.to_string()));
        try!(map.serialize_entry("name", &self.name));
        try!(map.serialize_entry("last_modified",
                                 &self.last_modified
                                     .to_utc()
                                     .strftime(if self.last_modified.tm_nsec == 0 {
                                         "%Y-%m-%dT%H:%M:%SZ"
                                     } else {
                                         "%Y-%m-%dT%H:%M:%S.%fZ"
                                     })
                                     .unwrap()
                                     .to_string()));
        try!(map.serialize_entry("size", &self.size));
        try!(map.serialize_entry("is_file", &self.is_file));
        map.end()
    }
}

impl<'de> Deserialize<'de> for RawFileData {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<RawFileData, D::Error> {
        deserializer.deserialize_struct("RawFileData", RAW_FILE_DATA_FIELDS, RawFileDataVisitor)
    }
}


struct RawFileDataVisitor;

impl<'de> Visitor<'de> for RawFileDataVisitor {
    type Value = RawFileData;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct RawFileData")
    }

    fn visit_seq<V: SeqAccess<'de>>(self, mut seq: V) -> Result<RawFileData, V::Error> {
        Ok(RawFileData {
            mime_type: {
                let mt: String = try!(try!(seq.next_element()).ok_or_else(|| de::Error::invalid_length(0, &self)));
                try!(mt.parse()
                    .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(&mt), &"valid MIME type")))
            },
            name: try!(try!(seq.next_element()).ok_or_else(|| de::Error::invalid_length(1, &self))),
            last_modified: {
                let lm: String = try!(try!(seq.next_element()).ok_or_else(|| de::Error::invalid_length(0, &self)));
                try!(parse_rfc3339(&lm).map_err(|_| de::Error::invalid_value(de::Unexpected::Str(&lm), &"RRC3339 timestamp")))
            },
            size: try!(try!(seq.next_element()).ok_or_else(|| de::Error::invalid_length(3, &self))),
            is_file: try!(try!(seq.next_element()).ok_or_else(|| de::Error::invalid_length(4, &self))),
        })
    }

    fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<RawFileData, V::Error> {
        let mut mime_type = None;
        let mut name = None;
        let mut last_modified = None;
        let mut size = None;
        let mut is_file = None;
        while let Some(key) = try!(map.next_key::<String>()) {
            match &key[..] {
                "mime_type" => {
                    if mime_type.is_some() {
                        return Err(de::Error::duplicate_field("mime_type"));
                    }
                    let nv: String = try!(map.next_value());
                    mime_type = Some(try!(nv.parse::<Mime>()
                        .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(&nv), &"valid MIME type"))));
                }
                "name" => {
                    if name.is_some() {
                        return Err(de::Error::duplicate_field("name"));
                    }
                    name = Some(try!(map.next_value()));
                }
                "last_modified" => {
                    if last_modified.is_some() {
                        return Err(de::Error::duplicate_field("last_modified"));
                    }
                    let nv: String = try!(map.next_value());
                    last_modified = Some(try!(parse_rfc3339(&nv).map_err(|_| de::Error::invalid_value(de::Unexpected::Str(&nv), &"RRC3339 timestamp"))));
                }
                "size" => {
                    if size.is_some() {
                        return Err(de::Error::duplicate_field("size"));
                    }
                    size = Some(try!(map.next_value()));
                }
                "is_file" => {
                    if is_file.is_some() {
                        return Err(de::Error::duplicate_field("is_file"));
                    }
                    is_file = Some(try!(map.next_value()));
                }
                key => return Err(de::Error::unknown_field(key, RAW_FILE_DATA_FIELDS)),
            }
        }

        Ok(RawFileData {
            mime_type: try!(mime_type.ok_or_else(|| de::Error::missing_field("mime_type"))),
            name: try!(name.ok_or_else(|| de::Error::missing_field("name"))),
            last_modified: try!(last_modified.ok_or_else(|| de::Error::missing_field("last_modified"))),
            size: try!(size.ok_or_else(|| de::Error::missing_field("size"))),
            is_file: try!(is_file.ok_or_else(|| de::Error::missing_field("is_file"))),
        })
    }
}
