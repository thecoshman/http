use rfsapi::util::parse_rfc3339;
use chrono;


#[test]
fn from_local() {
    assert_eq!(parse_rfc3339("2013-02-05T17:20:46+02:00"),
               Ok(chrono::DateTime::<chrono::FixedOffset>::from_naive_utc_and_offset(chrono::NaiveDateTime::new(chrono::NaiveDate::from_ymd_opt(2013, 2, 5).unwrap(),
                                                                                                                chrono::NaiveTime::from_hms_opt(17 - 2, 20, 46).unwrap()),
                                                              chrono::FixedOffset::east_opt(2 * 60 * 60).unwrap())));
    assert_eq!(parse_rfc3339("2005-10-02T05:21:52.420526571Z"),
               Ok(chrono::DateTime::<chrono::FixedOffset>::from_naive_utc_and_offset(chrono::NaiveDateTime::new(chrono::NaiveDate::from_ymd_opt(2005, 10, 2).unwrap(),
                                                                                                                chrono::NaiveTime::from_hms_nano_opt(5, 21, 52, 420526571).unwrap()),
                                                              chrono::FixedOffset::west_opt(0).unwrap())));
}

#[test]
fn from_utc() {
    assert_eq!(parse_rfc3339("2014-11-28T15:12:51Z"),
               Ok(chrono::DateTime::<chrono::FixedOffset>::from_naive_utc_and_offset(chrono::NaiveDateTime::new(chrono::NaiveDate::from_ymd_opt(2014, 11, 28).unwrap(),
                                                                                                                chrono::NaiveTime::from_hms_opt(15, 12, 51).unwrap()),
                                                              chrono::FixedOffset::west_opt(0).unwrap())));
    assert_eq!(parse_rfc3339("2002-10-02T15:00:00.05Z"),
               Ok(chrono::DateTime::<chrono::FixedOffset>::from_naive_utc_and_offset(chrono::NaiveDateTime::new(chrono::NaiveDate::from_ymd_opt(2002, 10, 2).unwrap(),
                                                                                                                chrono::NaiveTime::from_hms_nano_opt(15, 0, 0, 50000000).unwrap()),
                                                              chrono::FixedOffset::west_opt(0).unwrap())));
}

#[test]
fn trans_local() {
    let tm = chrono::Local::now().into();
    assert_eq!(parse_rfc3339(tm.format("%Y-%m-%dT%H:%M:%S.%f%z")
                   .to_string()),
               Ok(tm));
}

#[test]
fn trans_utc() {
    let tm = chrono::Utc::now().into();
    assert_eq!(parse_rfc3339(tm.format("%Y-%m-%dT%H:%M:%S.%fZ")
                   .to_string()),
               Ok(tm));
}
