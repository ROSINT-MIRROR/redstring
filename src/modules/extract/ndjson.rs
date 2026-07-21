use crate::error::Error;

use super::{Extractor, Record, Records};

pub struct Ndjson;

impl Ndjson {
    pub fn from_config(_config: &ron::Value) -> Result<Self, Error> {
        Ok(Self)
    }
}

impl Extractor for Ndjson {
    fn extract(&self, stdout: &str) -> Result<Records, Error> {
        let mut records: Records = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(serde_json::Value::Object(obj)) =
                serde_json::from_str::<serde_json::Value>(line)
            else {
                continue;
            };
            let mut record: Record = Record::new();
            for (k, v) in &obj {
                let key = k.trim().to_lowercase();
                if key.is_empty() {
                    continue;
                }
                if let serde_json::Value::String(s) = v {
                    push_field(&mut record, &key, s);
                    continue;
                }
                let serde_json::Value::Array(items) = v else {
                    continue;
                };
                for item in items {
                    let serde_json::Value::String(s) = item else {
                        continue;
                    };
                    push_field(&mut record, &key, s);
                }
            }
            if !record.is_empty() {
                records.push(record);
            }
        }
        Ok(records)
    }
}

fn push_field(record: &mut Record, key: &str, val: &str) {
    let val = val.trim();
    if val.is_empty() {
        return;
    }
    let entry = record.entry(key.to_owned()).or_default();
    if !entry.iter().any(|e| e == val) {
        entry.push(val.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_record_per_object_and_junk_is_skipped() {
        let text = "\
{\"slug\":\"github\",\"url\":\"https://github.com/torvalds\"}
not json at all
{\"slug\":\"gitlab\",\"url\":\"https://gitlab.com/torvalds\"}
{\"slug\":\"empty\",\"url\":\"\"}
{\"count\":3}
";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records.len(), 3, "3 object lines carrying at least one usable field");
        assert_eq!(records[0]["slug"], vec!["github"]);
        assert_eq!(records[0]["url"], vec!["https://github.com/torvalds"]);
        assert_eq!(records[1]["slug"], vec!["gitlab"]);
        assert_eq!(records[2]["slug"], vec!["empty"]);
        assert!(!records[2].contains_key("url"), "empty value dropped");
    }

    #[test]
    fn array_field_becomes_multivalue_deduped() {
        let text = "{\"name\":\"x\",\"tags\":[\"a\",\"a\",\"b\",\"\"]}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["name"], vec!["x"]);
        assert_eq!(records[0]["tags"], vec!["a", "b"]);
    }
}
