use brotli::enc::backward_references::{BrotliEncoderParams, BrotliEncoderMode};
use brotli::enc::BrotliCompress as brotli_compress;
use flate2::write::{DeflateEncoder, GzEncoder};
use flate2::Compression as Flate2Compression;
use iron::headers::{QualityItem, Encoding};
use bzip2::Compression as BzCompression;
use std::collections::BTreeSet;
use bzip2::write::BzEncoder;
use std::io::{self, Write};
use unicase::UniCase;
use std::path::Path;
use std::fs::File;
use blake3;


lazy_static! {
    /// The list of content encodings we handle.
    pub static ref SUPPORTED_ENCODINGS: Vec<Encoding> = {
        let es = vec![Encoding::Gzip, Encoding::Deflate, Encoding::EncodingExt("br".to_string()), Encoding::EncodingExt("bzip2".to_string())];
        [es.clone(), es.into_iter().map(|e| Encoding::EncodingExt(format!("x-{}", e))).collect()].iter().flat_map(|e| e.clone()).collect()
    };

    /// The list of extensions not to encode.
    pub static ref BLACKLISTED_ENCODING_EXTENSIONS: BTreeSet<UniCase<&'static str>> = {
        let raw = include_str!("../../assets/encoding_blacklist");
        raw.split('\n').map(str::trim).filter(|s| !s.is_empty() && !s.starts_with('#')).map(UniCase::new).collect()
    };

    pub static ref BROTLI_PARAMS: BrotliEncoderParams = BrotliEncoderParams {
        mode: BrotliEncoderMode::BROTLI_MODE_TEXT,
        ..Default::default()
    };
}

/// The minimal size at which to encode filesystem files.
pub const MIN_ENCODING_SIZE: u64 = 1024;

/// The maximal size at which to encode filesystem files.
pub const MAX_ENCODING_SIZE: u64 = 100 * 1024 * 1024;

/// The minimal size gain at which to preserve encoded filesystem files.
pub const MIN_ENCODING_GAIN: f64 = 1.1;


/// Find best supported encoding to use, or `None` for identity.
pub fn response_encoding(requested: &mut [QualityItem<Encoding>]) -> Option<Encoding> {
    requested.sort_by_key(|e| e.quality);
    requested.iter().filter(|e| e.quality.0 != 0).find(|e| SUPPORTED_ENCODINGS.contains(&e.item)).map(|e| e.item.clone())
}

/// Encode a string slice using a specified encoding or `None` if encoding failed or is not recognised.
pub fn encode_str(dt: &str, enc: &Encoding) -> Option<Vec<u8>> {
    type EncodeT = fn(&str) -> Option<Vec<u8>>;
    const STR_ENCODING_FNS: &[EncodeT] = &[encode_str_gzip, encode_str_deflate, encode_str_brotli, encode_str_bzip2];

    encoding_idx(enc).and_then(|fi| STR_ENCODING_FNS[fi](dt))
}

/// Encode the file denoted by the specified path into the file denoted by the specified path using a specified encoding or
/// `false` if encoding failed, is not recognised or an I/O error occurred.
pub fn encode_file(p: &Path, op: &Path, enc: &Encoding) -> bool {
    type EncodeT = fn(File, File) -> bool;
    const FILE_ENCODING_FNS: &[EncodeT] = &[encode_file_gzip, encode_file_deflate, encode_file_brotli, encode_file_bzip2];

    encoding_idx(enc)
        .map(|fi| {
            let inf = File::open(p);
            let outf = File::create(op);

            inf.is_ok() && outf.is_ok() && FILE_ENCODING_FNS[fi](inf.unwrap(), outf.unwrap())
        })
        .unwrap()
}

/// Encoding extension to use for encoded files, for example "gz" for gzip, or `None` if the encoding is not recognised.
pub fn encoding_extension(enc: &Encoding) -> Option<&'static str> {
    const ENCODING_EXTS: &[&str] = &["gz", "dflt", "br", "bz2"];

    encoding_idx(enc).map(|ei| ENCODING_EXTS[ei])
}

/// Return the 256-bit BLAKE3 hash of the file denoted by the specified path.
pub fn file_hash(p: &Path) -> blake3::Hash {
    let mut ctx = blake3::Hasher::new();
    io::copy(&mut File::open(p).unwrap(), &mut ctx).unwrap();
    ctx.finalize()
}


fn encoding_idx(enc: &Encoding) -> Option<usize> {
    match *enc {
        Encoding::Gzip => Some(0),
        Encoding::Deflate => Some(1),
        Encoding::EncodingExt(ref e) => {
            match &e[..] {
                "x-gzip" => Some(0),
                "x-deflate" => Some(1),
                "br" | "x-br" => Some(2),
                "bzip2" | "x-bzip2" => Some(3),
                _ => None,
            }
        }
        _ => None,
    }
}

macro_rules! encode_fn {
    ($str_fn_name:ident, $file_fn_name:ident, $enc_tp:ident, $comp_lvl:expr, $constructor:expr) => {
        fn $str_fn_name(dt: &str) -> Option<Vec<u8>> {
            let mut cmp = $constructor(Vec::new());
            cmp.write_all(dt.as_bytes()).ok().and_then(|_| cmp.finish().ok())
        }

        fn $file_fn_name(mut inf: File, outf: File) -> bool {
            let mut cmp = $constructor(outf);
            io::copy(&mut inf, &mut cmp).and_then(|_| cmp.finish()).is_ok()
        }
    };

    ($str_fn_name:ident, $file_fn_name:ident, $enc_tp:ident, $comp_lvl:expr) => {
        encode_fn!($str_fn_name, $file_fn_name, $enc_tp, $comp_lvl, |into| $enc_tp::new(into, $comp_lvl));
    }
}

encode_fn!(encode_str_gzip, encode_file_gzip, GzEncoder, Flate2Compression::default());
encode_fn!(encode_str_deflate, encode_file_deflate, DeflateEncoder, Flate2Compression::default());
encode_fn!(encode_str_bzip2, encode_file_bzip2, BzEncoder, BzCompression::Default);

fn encode_str_brotli(dt: &str) -> Option<Vec<u8>> {
    let mut ret = Vec::new();
    brotli_compress(&mut dt.as_bytes(), &mut ret, &BROTLI_PARAMS).ok().map(|_| ret)
}

fn encode_file_brotli(mut inf: File, mut outf: File) -> bool {
    brotli_compress(&mut inf, &mut outf, &BROTLI_PARAMS).is_ok()
}
