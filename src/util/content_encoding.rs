use iron::headers::{QualityItem, Encoding};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;


/// The list of content encodings we handle.
pub static SUPPORTED_ENCODINGS: &'static [Encoding] = &[Encoding::Gzip];


/// Find best supported encoding to use, or `None` for identity.
pub fn response_encoding(requested: &mut [QualityItem<Encoding>]) -> Option<Encoding> {
    requested.sort_by_key(|e| e.quality);
    requested.iter().filter(|e| e.quality.0 != 0).find(|e| SUPPORTED_ENCODINGS.contains(&e.item)).map(|e| e.item.clone())
}

/// Encode a string slice using a given encoding or `None` if encoding failed or is not recognised.
pub fn encode_str(dt: &str, enc: &Encoding) -> Option<Vec<u8>> {
    match *enc {
        Encoding::Gzip => encode_gzip(dt),
        _ => None,
    }
}

fn encode_gzip(dt: &str) -> Option<Vec<u8>> {
    let mut cmp = GzEncoder::new(Vec::new(), Compression::Default);
    cmp.write_all(dt.as_bytes()).ok().and_then(|_| cmp.finish().ok())
}
