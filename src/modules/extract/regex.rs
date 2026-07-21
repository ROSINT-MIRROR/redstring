use regex::Regex;
use serde::Deserialize;

use crate::error::Error;

use super::{Extractor, Record, Records};

#[derive(Deserialize)]
struct RegexConfig {
    pattern: String,
}

pub struct RegexExtract {
    re: Regex,
}

impl RegexExtract {
    pub fn from_config(config: &ron::Value) -> Result<Self, Error> {
        let cfg =
            config
                .clone()
                .into_rust::<RegexConfig>()
                .map_err(|source| Error::BadExtractConfig {
                    id: "regex".to_owned(),
                    source,
                })?;
        let re = Regex::new(&cfg.pattern).map_err(|source| Error::BadRegex {
            pattern: cfg.pattern.clone(),
            source,
        })?;
        Ok(Self { re })
    }
}

impl Extractor for RegexExtract {
    fn extract(&self, stdout: &str) -> Result<Records, Error> {
        let names: Vec<&str> = self.re.capture_names().flatten().collect();
        let mut records: Records = Vec::new();
        for caps in self.re.captures_iter(stdout) {
            let mut record = Record::new();
            if names.is_empty() {
                let Some(m) = caps.get(0) else {
                    continue;
                };
                let val = m.as_str().trim();
                if val.is_empty() {
                    continue;
                }
                record.entry("match".to_owned()).or_default().push(val.to_owned());
            } else {
                for &name in &names {
                    let Some(m) = caps.name(name) else {
                        continue;
                    };
                    let val = m.as_str().trim();
                    if val.is_empty() {
                        continue;
                    }
                    record
                        .entry(name.to_lowercase())
                        .or_default()
                        .push(val.to_owned());
                }
            }
            if !record.is_empty() {
                records.push(record);
            }
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(pattern: &str) -> RegexExtract {
        let cfg = ron::from_str::<ron::Value>(&format!("(pattern: {pattern:?})")).unwrap();
        RegexExtract::from_config(&cfg).unwrap()
    }

    #[test]
    fn named_group_yields_one_record_per_match() {
        let re = build("(?<id>G-[A-Z0-9]+|GTM-[A-Z0-9]+)");
        let text = "head G-ABC123 middle\nGTM-XYZ999 tail\n";
        let records = re.extract(text).unwrap();
        assert_eq!(records.len(), 2, "one record per match across lines");
        assert_eq!(records[0]["id"], vec!["G-ABC123"]);
        assert_eq!(records[1]["id"], vec!["GTM-XYZ999"]);
    }

    #[test]
    fn multiple_named_groups_map_to_fields() {
        let re = build("(?<user>[a-z]+)@(?<host>[a-z.]+)");
        let records = re.extract("mail alice@corp.com now").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["user"], vec!["alice"]);
        assert_eq!(records[0]["host"], vec!["corp.com"]);
    }

    #[test]
    fn no_named_group_falls_back_to_match() {
        let re = build("[a-f0-9]{8}");
        let records = re.extract("deadbeef and cafed00d here").unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["match"], vec!["deadbeef"]);
        assert_eq!(records[1]["match"], vec!["cafed00d"]);
        assert!(!records[0].contains_key("id"));
    }

    #[test]
    fn bad_pattern_is_an_error() {
        let cfg = ron::from_str::<ron::Value>("(pattern: \"(unclosed\")").unwrap();
        assert!(matches!(
            RegexExtract::from_config(&cfg),
            Err(Error::BadRegex { .. })
        ));
    }
}
