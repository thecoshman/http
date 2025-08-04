use std::str::FromStr;
use std::fmt::{self, Display};

use chrono::{self, TimeZone, Utc};

/// A `time::Time` with HTTP formatting and parsing
///
//   Prior to 1995, there were three different formats commonly used by
//   servers to communicate timestamps.  For compatibility with old
//   implementations, all three are defined here.  The preferred format is
//   a fixed-length and single-zone subset of the date and time
//   specification used by the Internet Message Format [RFC5322].
//
//     HTTP-date    = IMF-fixdate / obs-date
//
//   An example of the preferred format is
//
//     Sun, 06 Nov 1994 08:49:37 GMT    ; IMF-fixdate
//
//   Examples of the two obsolete formats are
//
//     Sunday, 06-Nov-94 08:49:37 GMT   ; obsolete RFC 850 format
//     Sun Nov  6 08:49:37 1994         ; ANSI C's asctime() format
//
//   A recipient that parses a timestamp value in an HTTP header field
//   MUST accept all three HTTP-date formats.  When a sender generates a
//   header field that contains one or more timestamps defined as
//   HTTP-date, the sender MUST generate those timestamps in the
//   IMF-fixdate format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HttpDate(pub chrono::DateTime<chrono::FixedOffset>);

impl FromStr for HttpDate {
    type Err = ::Error;
    fn from_str(s: &str) -> ::Result<HttpDate> {
        match chrono::NaiveDateTime::parse_from_str(s, "%a, %d %b %Y %T %Z").or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(s, "%A, %d-%b-%y %T %Z")
            }).or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(s, "%c")
                }) {
                    Ok(t) => Ok(HttpDate(Utc.from_utc_datetime(&t).into())),
                    Err(_) => Err(::Error::Header),
                    }
    }
}

impl Display for HttpDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut stamp = self.0.to_utc().to_rfc2822(); // in "Mon, 4 Aug 2025 18:27:18 +0000" format; we need s/+0000/GMT/
        stamp.replace_range(stamp.len() - 5.., "GMT");
        f.write_str(&stamp)
    }
}

#[cfg(test)]
mod tests {
    use chrono;
    use super::HttpDate;

    const NOV_07: HttpDate = HttpDate(chrono::Utc.from_utc_datetime(&chrono::NaiveDateTime::new(chrono::NaiveDate::from_ymd_opt(1994, 11, 7).unwrap(),
                                                                                                chrono::NaiveTime::from_hms_opt(8, 48, 37).unwrap())).into());

    #[test]
    fn test_imf_fixdate() {
        assert_eq!("Mon, 07 Nov 1994 08:48:37 GMT".parse::<HttpDate>().unwrap(), NOV_07);
    }

    #[test]
    fn test_rfc_850() {
        assert_eq!("Monday, 07-Nov-94 08:48:37 GMT".parse::<HttpDate>().unwrap(), NOV_07);
    }

    #[test]
    fn test_asctime() {
        assert_eq!("Mon Nov  7 08:48:37 1994".parse::<HttpDate>().unwrap(), NOV_07);
    }

    #[test]
    fn test_no_date() {
        assert!("this-is-no-date".parse::<HttpDate>().is_err());
    }
}
