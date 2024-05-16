use brotli::enc::backward_references::{BrotliEncoderParams, BrotliEncoderMode};
use std::io::{self, BufReader, BufWriter, Error as IoError, Write};
use iron::headers::{QualityItem, EncodingType, Encoding};
use brotli::enc::BrotliCompress as brotli_compress;
use flate2::write::{DeflateEncoder, GzEncoder};
use flate2::Compression as Flate2Compression;
use bzip2::write::BzEncoder;
use std::path::Path;
use std::ffi::OsStr;
use std::fs::File;
use blake3;


/// The minimal size at which to encode filesystem files.
pub const MIN_ENCODING_SIZE: u64 = 1024;

/// The maximal size at which to encode filesystem files.
pub const MAX_ENCODING_SIZE: u64 = 100 * 1024 * 1024;

/// The minimal size gain at which to preserve encoded filesystem files.
pub const MIN_ENCODING_GAIN: f64 = 1.1;


// `true` if we know not to encode the given extension
// pub fn extension_is_blacklisted(ext: &str) -> bool {
include!(concat!(env!("OUT_DIR"), "/extensions.rs"));


/// Find best supported encoding to use, or `None` for identity.
pub fn response_encoding(requested: &mut [QualityItem<Encoding>]) -> Option<Encoding> {
    requested.sort_by_key(|e| e.quality);
    requested.iter().filter(|e| e.quality.0 != 0).map(|e| &e.item).find(|e| encoding_idx(e).is_some()).cloned()
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
pub fn file_hash(p: &Path) -> Result<blake3::Hash, IoError> {
    let mut ctx = blake3::Hasher::new();
    io::copy(&mut BufReader::with_capacity(1024 * 1024, File::open(p)?), &mut ctx)?;
    Ok(ctx.finalize())
}


fn encoding_idx(enc: &Encoding) -> Option<usize> {
    match enc.0 {
        EncodingType::Gzip => Some(0),
        EncodingType::Deflate => Some(1),
        EncodingType::Brotli => Some(2),
        EncodingType::Bzip2 => Some(3),
        _ => None,
    }
}

macro_rules! encode_fn {
    ($str_fn_name:ident, $file_fn_name:ident, $enc_tp:ident, $comp_lvl:expr, $constructor:expr) => {
        fn $str_fn_name(dt: &str) -> Option<Vec<u8>> {
            let mut cmp = $constructor(Vec::new());
            cmp.write_all(dt.as_bytes()).ok().and_then(|_| cmp.finish().ok())
        }

        fn $file_fn_name(inf: File, outf: File) -> bool {
            let mut cmp = $constructor(BufWriter::with_capacity(1024 * 1024, outf));
            io::copy(&mut BufReader::with_capacity(1024 * 1024, inf), &mut cmp).and_then(|_| cmp.finish()).is_ok()
        }
    };

    ($str_fn_name:ident, $file_fn_name:ident, $enc_tp:ident, $comp_lvl:expr) => {
        encode_fn!($str_fn_name, $file_fn_name, $enc_tp, $comp_lvl, |into| $enc_tp::new(into, $comp_lvl));
    }
}

encode_fn!(encode_str_gzip, encode_file_gzip, GzEncoder, Flate2Compression::default());
encode_fn!(encode_str_deflate, encode_file_deflate, DeflateEncoder, Flate2Compression::default());
encode_fn!(encode_str_bzip2, encode_file_bzip2, BzEncoder, Default::default());

/// This should just be a pub const, but the new and default functions aren't const
pub fn brotli_params() -> BrotliEncoderParams {
    BrotliEncoderParams { mode: BrotliEncoderMode::BROTLI_MODE_TEXT, quality: 9, ..Default::default() }
}
fn encode_str_brotli(dt: &str) -> Option<Vec<u8>> {
    let mut ret = Vec::new();
    brotli_compress(&mut dt.as_bytes(), &mut ret, &brotli_params()).ok().map(|_| ret)
}
fn encode_file_brotli(inf: File, outf: File) -> bool {
    brotli_compress(&mut BufReader::with_capacity(1024 * 1024, inf), &mut BufWriter::with_capacity(1024 * 1024, outf), &brotli_params()).is_ok()
}
