use rfsapi::util::parse_rfc3339;
use serde_json::{self, Value};
use rfsapi::RawFileData;


#[test]
fn serialize() {
    assert_eq!(serde_json::to_value(RawFileData {
                                        mime_type: "text/plain".parse().unwrap(),
                                        name: "capitalism.txt".to_string(),
                                        last_modified: parse_rfc3339("2013-02-05T16:20:46Z").unwrap(),
                                        size: 1023,
                                        is_file: true,
                                    })
                       .unwrap(),
               Value::Object(vec![("mime_type".to_string(), Value::String("text/plain".to_string())),
                                  ("name".to_string(), Value::String("capitalism.txt".to_string())),
                                  ("last_modified".to_string(), Value::String("2013-02-05T16:20:46Z".to_string())),
                                  ("size".to_string(), Value::Number(1023.into())),
                                  ("is_file".to_string(), Value::Bool(true))]
                                     .into_iter()
                                     .collect()));
}

#[test]
fn deserialize() {
    assert_eq!(serde_json::from_value::<RawFileData>(Value::Object(vec![("mime_type".to_string(), Value::String("text/directory".to_string())),
                                                                        ("name".to_string(), Value::String("kaschism".to_string())),
                                                                        ("last_modified".to_string(), Value::String("2013-02-05T16:20:46Z".to_string())),
                                                                        ("size".to_string(), Value::Number(0.into())),
                                                                        ("is_file".to_string(), Value::Bool(false))]
                                                                           .into_iter()
                                                                           .collect()))
                       .unwrap(),
               RawFileData {
                   mime_type: "text/directory".parse().unwrap(),
                   name: "kaschism".to_string(),
                   last_modified: parse_rfc3339("2013-02-05T16:20:46Z").unwrap(),
                   size: 0,
                   is_file: false,
               });
}
