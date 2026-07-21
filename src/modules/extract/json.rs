use serde::Deserialize;

use crate::error::Error;

use super::{Extractor, Record, Records};

#[derive(Deserialize)]
struct JsonConfig {
    #[serde(default)]
    array: String,
    #[serde(default)]
    fields: Vec<(String, String)>,
}

pub struct Json {
    array: String,
    fields: Vec<(String, String)>,
}

impl Json {
    pub fn from_config(config: &ron::Value) -> Result<Self, Error> {
        let cfg = match config {
            ron::Value::Unit => JsonConfig {
                array: String::new(),
                fields: Vec::new(),
            },
            other => other.clone().into_rust::<JsonConfig>().map_err(|source| {
                Error::BadExtractConfig {
                    id: "json".to_owned(),
                    source,
                }
            })?,
        };
        Ok(Self {
            array: cfg.array,
            fields: cfg.fields,
        })
    }
}

impl Extractor for Json {
    fn extract(&self, stdout: &str) -> Result<Records, Error> {
        let Ok(root) = serde_json::from_str::<serde_json::Value>(stdout) else {
            return Ok(Vec::new());
        };
        let Some(target) = locate(&root, &self.array) else {
            return Ok(Vec::new());
        };
        let elements: Vec<&serde_json::Value> = if let serde_json::Value::Array(items) = target {
            items.iter().collect()
        } else if let serde_json::Value::Object(_) = target {
            vec![target]
        } else {
            return Ok(Vec::new());
        };
        let mut records: Records = Vec::new();
        for el in elements {
            let record = build_record(el, &self.fields);
            if !record.is_empty() {
                records.push(record);
            }
        }
        Ok(records)
    }
}

fn locate<'a>(root: &'a serde_json::Value, array: &str) -> Option<&'a serde_json::Value> {
    if array.is_empty() {
        return Some(root);
    }
    navigate(root, array)
}

fn navigate<'a>(root: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cur = root;
    for seg in path.split('.') {
        if seg.is_empty() {
            continue;
        }
        if let serde_json::Value::Object(m) = cur {
            cur = m.get(seg)?;
            continue;
        }
        if let serde_json::Value::Array(a) = cur {
            let Ok(idx) = seg.parse::<usize>() else {
                return None;
            };
            cur = a.get(idx)?;
            continue;
        }
        return None;
    }
    Some(cur)
}

fn build_record(el: &serde_json::Value, fields: &[(String, String)]) -> Record {
    if fields.is_empty() {
        return flatten_object(el);
    }
    record_from_fields(el, fields)
}

fn record_from_fields(el: &serde_json::Value, fields: &[(String, String)]) -> Record {
    let mut record = Record::new();
    for (out, pointer) in fields {
        let key = out.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }
        let Some(v) = navigate(el, pointer) else {
            continue;
        };
        push_value(&mut record, &key, v);
    }
    record
}

fn flatten_object(el: &serde_json::Value) -> Record {
    let mut record = Record::new();
    let serde_json::Value::Object(obj) = el else {
        return record;
    };
    for (k, v) in obj {
        let key = k.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }
        push_value(&mut record, &key, v);
    }
    record
}

fn push_value(record: &mut Record, key: &str, value: &serde_json::Value) {
    if let serde_json::Value::Array(items) = value {
        for item in items {
            let text = scalar_text(item);
            push_field(record, key, &text);
        }
        return;
    }
    let text = scalar_text(value);
    push_field(record, key, &text);
}

fn scalar_text(value: &serde_json::Value) -> String {
    if let serde_json::Value::String(s) = value {
        return s.clone();
    }
    if let serde_json::Value::Number(n) = value {
        return n.to_string();
    }
    if let serde_json::Value::Bool(b) = value {
        return b.to_string();
    }
    String::new()
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
    fn crtsh_style_array_with_fields() {
        let cfg = ron::from_str::<ron::Value>(
            "(fields: [(\"issuer\", \"issuer_name\"), (\"domain\", \"name_value\")])",
        )
        .unwrap();
        let j = Json::from_config(&cfg).unwrap();
        let doc = "[{\"issuer_name\":\"Let's Encrypt\",\"name_value\":\"a.example.com\"},\
                   {\"issuer_name\":\"DigiCert\",\"name_value\":\"b.example.com\"}]";
        let records = j.extract(doc).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["issuer"], vec!["Let's Encrypt"]);
        assert_eq!(records[0]["domain"], vec!["a.example.com"]);
        assert_eq!(records[1]["issuer"], vec!["DigiCert"]);
        assert_eq!(records[1]["domain"], vec!["b.example.com"]);
    }

    #[test]
    fn nested_array_path_navigates_dot_path() {
        let cfg = ron::from_str::<ron::Value>("(array: \"result.certificates\")").unwrap();
        let j = Json::from_config(&cfg).unwrap();
        let doc = "{\"result\":{\"certificates\":[{\"cn\":\"x.com\"},{\"cn\":\"y.com\"}]}}";
        let records = j.extract(doc).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["cn"], vec!["x.com"]);
        assert_eq!(records[1]["cn"], vec!["y.com"]);
    }

    #[test]
    fn single_object_is_one_record_and_scalars_stringify() {
        let j = Json::from_config(&ron::Value::Unit).unwrap();
        let doc = "{\"asn\":\"AS15169\",\"count\":42,\"active\":true,\"nested\":{\"k\":1}}";
        let records = j.extract(doc).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["asn"], vec!["AS15169"]);
        assert_eq!(records[0]["count"], vec!["42"]);
        assert_eq!(records[0]["active"], vec!["true"]);
        assert!(!records[0].contains_key("nested"), "nested object skipped");
    }

    #[test]
    fn scalar_array_field_is_multivalued_and_deduped() {
        let cfg = ron::from_str::<ron::Value>("(fields: [(\"san\", \"names\")])").unwrap();
        let j = Json::from_config(&cfg).unwrap();
        let doc = "[{\"names\":[\"a.com\",\"a.com\",\"b.com\",\"\"]}]";
        let records = j.extract(doc).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["san"], vec!["a.com", "b.com"]);
    }

    #[test]
    fn indexed_path_into_array_element() {
        let cfg = ron::from_str::<ron::Value>("(array: \"hits.1\")").unwrap();
        let j = Json::from_config(&cfg).unwrap();
        let doc = "{\"hits\":[{\"h\":\"first\"},{\"h\":\"second\"}]}";
        let records = j.extract(doc).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["h"], vec!["second"]);
    }

    #[test]
    fn non_json_degrades_to_empty() {
        let j = Json::from_config(&ron::Value::Unit).unwrap();
        assert!(j.extract("<html>not json</html>").unwrap().is_empty());
    }

    #[test]
    fn missing_array_path_degrades_to_empty() {
        let cfg = ron::from_str::<ron::Value>("(array: \"nope.here\")").unwrap();
        let j = Json::from_config(&cfg).unwrap();
        assert!(j.extract("{\"result\":{}}").unwrap().is_empty());
    }
}
