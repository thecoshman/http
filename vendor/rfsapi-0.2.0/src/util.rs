//! Module containing various utility functions.


use time::{self, Tm};


/// Parse an RFC3339 string into a timespec.
///
/// Note: due to the specificity of the `tm` struct some fields are not
/// preserved, but have no impact on the correctness of the result:
///
/// * `tm_wday` – weekday
/// * `tm_yday` – day of the year
/// * `tm_isdst` – daylight savings time applied/not applied
///
/// # Examples
///
/// ```
/// # extern crate time;
/// # extern crate rfsapi;
/// # use time::Tm;
/// # use rfsapi::util::parse_rfc3339;
/// # fn main() {
/// assert_eq!(parse_rfc3339("2012-02-22T07:53:18-07:00"),
///            Ok(Tm {
///                tm_sec: 18,
///                tm_min: 53,
///                tm_hour: 7,
///                tm_mday: 22,
///                tm_mon: 1,
///                tm_year: 112,
///                tm_wday: 0,
///                tm_yday: 0,
///                tm_isdst: 0,
///                tm_utcoff: -25200,
///                tm_nsec: 0,
///            }));
/// assert_eq!(parse_rfc3339("2012-02-22T14:53:18.42Z"),
///            Ok(Tm {
///                tm_sec: 18,
///                tm_min: 53,
///                tm_hour: 14,
///                tm_mday: 22,
///                tm_mon: 1,
///                tm_year: 112,
///                tm_wday: 0,
///                tm_yday: 0,
///                tm_isdst: 0,
///                tm_utcoff: 0,
///                tm_nsec: 420000000,
///            }));
/// # }
/// ```
pub fn parse_rfc3339<S: AsRef<str>>(from: S) -> Result<Tm, time::ParseError> {
    let utc = from.as_ref().chars().last() == Some('Z');
    let fractional = from.as_ref().len() > if utc { 20 } else { 25 };
    time::strptime(from.as_ref(),
                   match (utc, fractional) {
                       (true, false) => "%Y-%m-%dT%H:%M:%SZ",
                       (true, true) => "%Y-%m-%dT%H:%M:%S.%fZ",
                       (false, true) => "%Y-%m-%dT%H:%M:%S.%f%z",
                       (false, false) => "%Y-%m-%dT%H:%M:%S%z",
                   })
}
