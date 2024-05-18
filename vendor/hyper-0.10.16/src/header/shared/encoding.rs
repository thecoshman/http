use std::fmt;
use std::str;

/// A value to represent an encoding used in `Transfer-Encoding`
/// or `Accept-Encoding` header.
///
/// bool is `x-`-prefixed.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Encoding(pub EncodingType, pub String, pub bool);
impl Encoding {
    #[allow(non_upper_case_globals)]
    pub const Chunked: Encoding = Encoding(EncodingType::Chunked, String::new(), false);
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EncodingType {
    /// The `chunked` encoding.
    Chunked,
    /// The `gzip` encoding.
    Gzip,
    /// The `deflate` encoding.
    Deflate,
    /// The `compress` encoding.
    Compress,
    /// The `identity` encoding.
    Identity,
    /// The `br` encoding.
    Brotli,
    /// The `bzip2` encoding.
    Bzip2,
    /// The `zstd` encoding.
    Zstd,
    /// See upper String.
    Custom,
}

impl fmt::Display for Encoding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.2 {
            f.write_str("x-")?;
        }
        f.write_str(match self.0 {
            EncodingType::Chunked => "chunked",
            EncodingType::Gzip => "gzip",
            EncodingType::Deflate => "deflate",
            EncodingType::Compress => "compress",
            EncodingType::Identity => "identity",
            EncodingType::Brotli => "br",
            EncodingType::Bzip2 => "bzip2",
            EncodingType::Zstd => "zstd",
            EncodingType::Custom => self.1.as_ref(),
        })
    }
}

impl str::FromStr for Encoding {
    type Err = ::Error;
    fn from_str(mut s: &str) -> ::Result<Encoding> {
        let x = s.starts_with("x-");
        if x {
            s = &s[2..];
        }
        let mut custom = String::new();
        let enc = match s {
            "chunked" => EncodingType::Chunked,
            "deflate" => EncodingType::Deflate,
            "gzip" => EncodingType::Gzip,
            "compress" => EncodingType::Compress,
            "identity" => EncodingType::Identity,
            "br" => EncodingType::Brotli,
            "bzip2" => EncodingType::Bzip2,
            "zstd" => EncodingType::Zstd,
            _ => {
                custom = s.to_owned();
                EncodingType::Custom
            },
        };
        Ok(Encoding(enc, custom, x))
    }
}
