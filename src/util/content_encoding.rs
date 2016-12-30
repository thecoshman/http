use iron::headers::{QualityItem, Encoding};


/// The list of content encodings we handle.
pub static SUPPORTED_ENCODINGS: &'static [Encoding] = &[];


/// Find best supported encoding to use, or `None` for identity.
pub fn response_encoding(requested: &mut [QualityItem<Encoding>]) -> Option<Encoding> {
    requested.sort_by_key(|e| e.quality);
    requested.iter().find(|e| SUPPORTED_ENCODINGS.contains(&e.item)).map(|e| e.item.clone())
}
