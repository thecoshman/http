use rfsapi::util::parse_rfc3339;
use time::{Tm, now_utc, now};


#[test]
fn from_local() {
    assert_eq!(parse_rfc3339("2013-02-05T17:20:46+02:00"),
               Ok(Tm {
                   tm_sec: 46,
                   tm_min: 20,
                   tm_hour: 17,
                   tm_mday: 5,
                   tm_mon: 1,
                   tm_year: 113,
                   tm_wday: 0,
                   tm_yday: 0,
                   tm_isdst: 0,
                   tm_utcoff: 7200,
                   tm_nsec: 0,
               }));
    assert_eq!(parse_rfc3339("2005-10-02T05:21:52.420526571Z"),
               Ok(Tm {
                   tm_sec: 52,
                   tm_min: 21,
                   tm_hour: 5,
                   tm_mday: 2,
                   tm_mon: 9,
                   tm_year: 105,
                   tm_wday: 0,
                   tm_yday: 0,
                   tm_isdst: 0,
                   tm_utcoff: 0,
                   tm_nsec: 420526571,
               }));
}

#[test]
fn from_utc() {
    assert_eq!(parse_rfc3339("2014-11-28T15:12:51Z"),
               Ok(Tm {
                   tm_sec: 51,
                   tm_min: 12,
                   tm_hour: 15,
                   tm_mday: 28,
                   tm_mon: 10,
                   tm_year: 114,
                   tm_wday: 0,
                   tm_yday: 0,
                   tm_isdst: 0,
                   tm_utcoff: 0,
                   tm_nsec: 0,
               }));
    assert_eq!(parse_rfc3339("2002-10-02T15:00:00.05Z"),
               Ok(Tm {
                   tm_sec: 0,
                   tm_min: 0,
                   tm_hour: 15,
                   tm_mday: 2,
                   tm_mon: 9,
                   tm_year: 102,
                   tm_wday: 0,
                   tm_yday: 0,
                   tm_isdst: 0,
                   tm_utcoff: 0,
                   tm_nsec: 50000000,
               }));
}

#[test]
fn trans_local() {
    let tm = Tm {
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        ..now()
    };
    assert_eq!(parse_rfc3339(tm.strftime("%Y-%m-%dT%H:%M:%S.%f%z")
                   .unwrap()
                   .to_string()),
               Ok(tm));
}

#[test]
fn trans_utc() {
    let tm = Tm {
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        ..now_utc()
    };
    assert_eq!(parse_rfc3339(tm.strftime("%Y-%m-%dT%H:%M:%S.%fZ")
                   .unwrap()
                   .to_string()),
               Ok(tm));
}
