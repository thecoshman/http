use hyper::header::{Raw as RawHeader, Header};
use hyper::Error as HyperError;
use rfsapi::RawFsApiHeader;


#[test]
fn header_name() {
    assert_eq!(RawFsApiHeader::header_name(), "X-Raw-Filesystem-API");
}

#[test]
fn parse_header_correct() {
    assert_eq!(RawFsApiHeader::parse_header(&RawHeader::from(vec![b'1'])).unwrap(), RawFsApiHeader(true));
    assert_eq!(RawFsApiHeader::parse_header(&RawHeader::from(vec![b'0'])).unwrap(), RawFsApiHeader(false));
}

#[test]
fn parse_header_incorrect() {
    assert_eq!(RawFsApiHeader::parse_header(&RawHeader::from(&b""[..])).unwrap_err().to_string(),
               HyperError::Header.to_string());
    assert_eq!(RawFsApiHeader::parse_header(&RawHeader::from(vec![vec![]])).unwrap_err().to_string(),
               HyperError::Header.to_string());
    assert_eq!(RawFsApiHeader::parse_header(&RawHeader::from(vec![vec![b'1', b'0']])).unwrap_err().to_string(),
               HyperError::Header.to_string());
    assert_eq!(RawFsApiHeader::parse_header(&RawHeader::from(vec![vec![b'1'], vec![b'1']])).unwrap_err().to_string(),
               HyperError::Header.to_string());
}

#[test]
fn fmt_header() {
    assert_eq!(&RawFsApiHeader(true).to_string(), "1");
    assert_eq!(&RawFsApiHeader(false).to_string(), "0");
}
