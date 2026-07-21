use serde::Deserialize;

use crate::error::Error;

use super::engine::norm_key;
use super::{Extractor, Record, Records};

#[derive(Deserialize)]
struct LinesConfig {
    #[serde(default = "default_field")]
    field: String,
}

fn default_field() -> String {
    "line".to_owned()
}

pub struct Lines {
    field: String,
}

impl Lines {
    pub fn from_config(config: &ron::Value) -> Result<Self, Error> {
        let raw = match config {
            ron::Value::Unit => default_field(),
            other => {
                other
                    .clone()
                    .into_rust::<LinesConfig>()
                    .map_err(|source| Error::BadExtractConfig {
                        id: "lines".to_owned(),
                        source,
                    })?
                    .field
            }
        };
        let field = norm_key(&raw)?;
        let field = if field.is_empty() {
            default_field()
        } else {
            field
        };
        Ok(Self { field })
    }
}

impl Extractor for Lines {
    fn extract(&self, stdout: &str) -> Result<Records, Error> {
        let mut records: Records = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut record = Record::new();
            record
                .entry(self.field.clone())
                .or_default()
                .push(line.to_owned());
            records.push(record);
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_nonempty_line_is_a_record() {
        let text = "  api.example.com  \n\nwww.example.com\n   \nvpn.example.com\n";
        let records = Lines {
            field: "domain".to_owned(),
        }
        .extract(text)
        .unwrap();
        assert_eq!(records.len(), 3, "three non-empty lines, blanks skipped");
        assert_eq!(records[0]["domain"], vec!["api.example.com"]);
        assert_eq!(records[1]["domain"], vec!["www.example.com"]);
        assert_eq!(records[2]["domain"], vec!["vpn.example.com"]);
    }

    #[test]
    fn default_field_is_line_and_unit_config_builds() {
        let built = Lines::from_config(&ron::Value::Unit).unwrap();
        assert_eq!(built.field, "line");
        let records = built.extract("only-one\n").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["line"], vec!["only-one"]);
    }

    #[test]
    fn config_sets_and_lowercases_field() {
        let cfg = ron::from_str::<ron::Value>("(field: \"URL\")").unwrap();
        let built = Lines::from_config(&cfg).unwrap();
        assert_eq!(built.field, "url");
    }

    #[test]
    fn empty_stdout_yields_no_records() {
        let records = Lines::from_config(&ron::Value::Unit)
            .unwrap()
            .extract("\n  \n\n")
            .unwrap();
        assert!(records.is_empty());
    }
}
