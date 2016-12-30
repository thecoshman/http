use brotli2::stream::{CompressMode as BrotliCompressMode, CompressParams as BrotliCompressParams};
use flate2::write::{DeflateEncoder, GzEncoder};
use flate2::Compression as Flate2Compression;
use bzip2::Compression as BzCompression;
use iron::headers::{QualityItem, Encoding};
use brotli2::write::BrotliEncoder;
use bzip2::write::BzEncoder;
use std::io::Write;


lazy_static! {
    /// The list of content encodings we handle.
    pub static ref SUPPORTED_ENCODINGS: Vec<Encoding> = {
        let es = vec![Encoding::Gzip, Encoding::Deflate, Encoding::EncodingExt("br".to_string()), Encoding::EncodingExt("bzip2".to_string())];
        [es.clone(), es.into_iter().map(|e| Encoding::EncodingExt(format!("x-{}", e))).collect()].into_iter().flat_map(|e| e.clone()).collect()
    };
}


/// Find best supported encoding to use, or `None` for identity.
pub fn response_encoding(requested: &mut [QualityItem<Encoding>]) -> Option<Encoding> {
    requested.sort_by_key(|e| e.quality);
    requested.iter().filter(|e| e.quality.0 != 0).find(|e| SUPPORTED_ENCODINGS.contains(&e.item)).map(|e| e.item.clone())
}

/// Encode a string slice using a given encoding or `None` if encoding failed or is not recognised.
pub fn encode_str(dt: &str, enc: &Encoding) -> Option<Vec<u8>> {
    match *enc {
        Encoding::Gzip => encode_gzip(dt),
        Encoding::Deflate => encode_deflate(dt),
        Encoding::EncodingExt(ref e) => {
            match &e[..] {
                "x-gzip" => encode_gzip(dt),
                "x-deflate" => encode_deflate(dt),
                "br" | "x-br" => encode_brotli(dt),
                "bzip2" | "x-bzip2" => encode_bzip2(dt),
                _ => None,
            }
        }
        _ => None,
    }
}


macro_rules! encode_fn_flate2_write_iface {
    ($fn_name:ident, $enc_tp:ident) => {
        fn $fn_name(dt: &str) -> Option<Vec<u8>> {
            let mut cmp = $enc_tp::new(Vec::new(), Flate2Compression::Default);
            cmp.write_all(dt.as_bytes()).ok().and_then(|_| cmp.finish().ok())
        }
    }
}

encode_fn_flate2_write_iface!(encode_gzip, GzEncoder);
encode_fn_flate2_write_iface!(encode_deflate, DeflateEncoder);

fn encode_brotli(dt: &str) -> Option<Vec<u8>> {
    let mut cmp = BrotliEncoder::new(Vec::new(), 0);
    cmp.set_params(BrotliCompressParams::new().mode(BrotliCompressMode::Text));
    cmp.write_all(dt.as_bytes()).ok().and_then(|_| cmp.finish().ok())
}

fn encode_bzip2(dt: &str) -> Option<Vec<u8>> {
    let mut cmp = BzEncoder::new(Vec::new(), BzCompression::Default);
    cmp.write_all(dt.as_bytes()).ok().and_then(|_| cmp.finish().ok())
}
