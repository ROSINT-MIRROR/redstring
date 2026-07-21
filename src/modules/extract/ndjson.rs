use crate::error::Error;

use super::engine::{norm_key, push_dedup};
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
                let key = norm_key(k)?;
                if key.is_empty() {
                    continue;
                }
                absorb(&mut record, &key, v, true)?;
            }
            if !record.is_empty() {
                records.push(record);
            }
        }
        Ok(records)
    }
}

fn absorb(record: &mut Record, key: &str, value: &serde_json::Value, nest: bool) -> Result<(), Error> {
    if push_scalar(record, key, value)? {
        return Ok(());
    }
    if let serde_json::Value::Array(items) = value {
        for item in items {
            push_scalar(record, key, item)?;
        }
        return Ok(());
    }
    if !nest {
        return Ok(());
    }
    let serde_json::Value::Object(inner) = value else {
        return Ok(());
    };
    for (child_key, child) in inner {
        let sub = norm_key(child_key)?;
        if sub.is_empty() {
            continue;
        }
        let dotted = format!("{key}.{sub}");
        absorb(record, &dotted, child, false)?;
    }
    Ok(())
}

fn push_scalar(record: &mut Record, key: &str, value: &serde_json::Value) -> Result<bool, Error> {
    if let serde_json::Value::String(s) = value {
        push_dedup(record, key, s)?;
        return Ok(true);
    }
    if let serde_json::Value::Number(n) = value {
        push_dedup(record, key, &number_text(n))?;
        return Ok(true);
    }
    if let serde_json::Value::Bool(b) = value {
        push_dedup(record, key, &b.to_string())?;
        return Ok(true);
    }
    Ok(false)
}

fn number_text(n: &serde_json::Number) -> String {
    if n.is_f64() {
        return float_text(n);
    }
    n.to_string()
}

fn float_text(n: &serde_json::Number) -> String {
    let Some(f) = n.as_f64() else {
        return n.to_string();
    };
    f.to_string()
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
    fn number_field_stringifies() {
        let text = "{\"host\":\"a.com\",\"status_code\":200,\"ratio\":1.5}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["host"], vec!["a.com"]);
        assert_eq!(records[0]["status_code"], vec!["200"]);
        assert_eq!(records[0]["ratio"], vec!["1.5"]);
    }

    #[test]
    fn large_integer_has_no_scientific_notation() {
        let text = "{\"asn\":20000000000}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records[0]["asn"], vec!["20000000000"]);
    }

    #[test]
    fn bool_field_stringifies() {
        let text = "{\"cdn\":true,\"wildcard\":false}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records[0]["cdn"], vec!["true"]);
        assert_eq!(records[0]["wildcard"], vec!["false"]);
    }

    #[test]
    fn nested_object_flattens_one_level_with_lowercased_keys() {
        let text = "{\"host\":\"a.com\",\"info\":{\"Severity\":\"high\",\"count\":3}}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records[0]["host"], vec!["a.com"]);
        assert_eq!(records[0]["info.severity"], vec!["high"]);
        assert_eq!(records[0]["info.count"], vec!["3"]);
        assert!(!records[0].contains_key("info"), "parent object key not emitted raw");
    }

    #[test]
    fn nested_object_stops_at_one_level() {
        let text = "{\"info\":{\"severity\":\"high\",\"meta\":{\"x\":\"y\"}}}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records[0]["info.severity"], vec!["high"]);
        assert!(!records[0].contains_key("info.meta"), "second level object skipped");
        assert!(!records[0].contains_key("info.meta.x"), "no two-level flatten");
    }

    #[test]
    fn scalar_array_stays_multivalue_and_deduped() {
        let text = "{\"name\":\"x\",\"tags\":[\"a\",\"a\",\"b\",\"\"]}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["name"], vec!["x"]);
        assert_eq!(records[0]["tags"], vec!["a", "b"]);
    }

    #[test]
    fn numeric_array_stringifies_each_and_dedupes() {
        let text = "{\"codes\":[200,200,404]}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records[0]["codes"], vec!["200", "404"]);
    }

    #[test]
    fn array_of_objects_is_skipped() {
        let text = "{\"host\":\"a.com\",\"hits\":[{\"k\":\"v\"},{\"k\":\"w\"}]}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records[0]["host"], vec!["a.com"]);
        assert!(!records[0].contains_key("hits"), "array of objects contributes nothing");
    }

    #[test]
    fn line_with_only_array_of_objects_is_dropped() {
        let text = "{\"hits\":[{\"k\":\"v\"}]}\n";
        let records = Ndjson.extract(text).unwrap();
        assert!(records.is_empty(), "no usable field means no record");
    }

    #[test]
    fn null_value_is_dropped() {
        let text = "{\"host\":\"a.com\",\"asn\":null}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records[0]["host"], vec!["a.com"]);
        assert!(!records[0].contains_key("asn"), "null carries no value");
    }

    #[test]
    fn pre_flattened_string_input_is_unchanged() {
        let text = "{\"slug\":\"github\",\"tags\":[\"a\",\"b\"],\"status_code\":\"200\"}\n";
        let records = Ndjson.extract(text).unwrap();
        assert_eq!(records[0]["slug"], vec!["github"]);
        assert_eq!(records[0]["tags"], vec!["a", "b"]);
        assert_eq!(records[0]["status_code"], vec!["200"]);
    }
}
