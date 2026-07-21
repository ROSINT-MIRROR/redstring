use serde::Deserialize;

use crate::error::Error;

use super::{Extractor, Record, Records};

#[derive(Deserialize)]
struct KvConfig {
    sep: String,
    #[serde(default)]
    comments: String,
}

pub struct KeyValue {
    sep: String,
    comments: String,
}

impl KeyValue {
    pub fn from_config(config: &ron::Value) -> Result<Self, Error> {
        let cfg = config
            .clone()
            .into_rust::<KvConfig>()
            .map_err(|source| Error::BadExtractConfig {
                id: "keyvalue".to_owned(),
                source,
            })?;
        Ok(Self {
            sep: cfg.sep,
            comments: cfg.comments,
        })
    }
}

impl Extractor for KeyValue {
    fn extract(&self, stdout: &str) -> Result<Records, Error> {
        let mut record: Record = Record::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if !self.comments.is_empty() && line.starts_with(self.comments.as_str()) {
                continue;
            }
            let Some((k, v)) = line.split_once(self.sep.as_str()) else {
                continue;
            };
            let key = k.trim().to_lowercase();
            let val = v.trim();
            if key.is_empty() || val.is_empty() {
                continue;
            }
            let entry = record.entry(key).or_default();
            if !entry.iter().any(|e| e == val) {
                entry.push(val.to_owned());
            }
        }
        Ok(vec![record])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(sep: &str, comments: &str) -> KeyValue {
        KeyValue {
            sep: sep.to_owned(),
            comments: comments.to_owned(),
        }
    }

    #[test]
    fn one_record_trims_skips_and_collects() {
        let text = "\
% comment line
   Registrar: MarkMonitor Inc.
   Name Server: NS1.GOOGLE.COM
   Name Server: NS2.GOOGLE.COM
   Name Server: NS1.GOOGLE.COM
   Registrar URL: http://www.markmonitor.com
   OriginAS:
no separator here
";
        let records = kv(":", "%").extract(text).unwrap();
        assert_eq!(records.len(), 1, "keyvalue yields exactly one record");
        let r = &records[0];
        assert_eq!(r["registrar"], vec!["MarkMonitor Inc."]);
        assert_eq!(r["name server"], vec!["NS1.GOOGLE.COM", "NS2.GOOGLE.COM"]);
        assert_eq!(r["registrar url"], vec!["http://www.markmonitor.com"]);
        assert!(!r.contains_key("originas"), "empty values dropped");
        assert!(!r.contains_key("% comment line"));
    }

    #[test]
    fn first_separator_wins() {
        let records = kv(":", "")
            .extract("Ref: https://rdap.arin.net/registry/ip/8.8.8.0")
            .unwrap();
        assert_eq!(records[0]["ref"], vec!["https://rdap.arin.net/registry/ip/8.8.8.0"]);
    }

    #[test]
    fn from_config_reads_sep_and_default_comments() {
        let cfg = ron::from_str::<ron::Value>("(sep: \":\")").unwrap();
        let built = KeyValue::from_config(&cfg).unwrap();
        assert_eq!(built.sep, ":");
        assert_eq!(built.comments, "");
    }
}
