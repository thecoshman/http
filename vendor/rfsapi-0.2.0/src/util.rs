//! Module containing various utility functions.


use chrono;


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
/// # extern crate chrono;
/// # extern crate rfsapi;
/// # use rfsapi::util::parse_rfc3339;
/// # fn main() {
/// assert_eq!(parse_rfc3339("2012-02-22T07:53:18-07:00"),
///            Ok(chrono::DateTime::<chrono::FixedOffset>::from_naive_utc_and_offset(chrono::NaiveDateTime::new(chrono::NaiveDate::from_ymd_opt(2012, 2, 22).unwrap(),
///                                                                                                             chrono::NaiveTime::from_hms_opt(7 + 7, 53, 18).unwrap()),
///                                                           chrono::FixedOffset::west_opt(7 * 60 * 60).unwrap())));
/// assert_eq!(parse_rfc3339("2012-02-22T14:53:18.42Z"),
///            Ok(chrono::DateTime::<chrono::FixedOffset>::from_naive_utc_and_offset(chrono::NaiveDateTime::new(chrono::NaiveDate::from_ymd_opt(2012, 2, 22).unwrap(),
///                                                                                                             chrono::NaiveTime::from_hms_milli_opt(14, 53, 18, 420).unwrap()),
///                                                           chrono::FixedOffset::west_opt(0).unwrap())));
/// # }
/// ```
pub fn parse_rfc3339<S: AsRef<str>>(from: S) -> chrono::ParseResult<chrono::DateTime<chrono::FixedOffset>> {
    let utc = from.as_ref().chars().last() == Some('Z');
    let fractional = from.as_ref().len() > if utc { 20 } else { 25 };
    if utc {
        chrono::NaiveDateTime::parse_from_str(from.as_ref(),
                                              match fractional {
                                                  false => "%Y-%m-%dT%H:%M:%SZ",
                                                  true => "%Y-%m-%dT%H:%M:%S%.fZ",
                                              })
            .map(|ndt| ndt.and_utc().into())
    } else {
        chrono::DateTime::parse_from_str(from.as_ref(),
                                         match fractional {
                                             true => "%Y-%m-%dT%H:%M:%S%.f%z",
                                             false => "%Y-%m-%dT%H:%M:%S%z",
                                         })
    }
}
