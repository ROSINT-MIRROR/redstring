use std::collections::BTreeMap;

use crate::error::Error;

use super::super::spec::{EmitRule, Shape};
use super::keyvalue::KeyValue;
use super::ndjson::Ndjson;

pub type Record = BTreeMap<String, Vec<String>>;
pub type Records = Vec<Record>;

pub trait Extractor {
    fn extract(&self, stdout: &str) -> Result<Records, Error>;
}

pub fn extractor(id: &str, config: &ron::Value) -> Result<Box<dyn Extractor>, Error> {
    match id {
        "keyvalue" => Ok(Box::new(KeyValue::from_config(config)?)),
        "ndjson" => Ok(Box::new(Ndjson::from_config(config)?)),
        other => Err(Error::UnknownExtractor {
            id: other.to_owned(),
        }),
    }
}

fn shape_hits(shape: &Shape, records: &Records) -> Result<usize, Error> {
    let mut hits = 0usize;
    let mut seen: Vec<&str> = Vec::new();
    for rule in &shape.emit {
        let from = match rule {
            EmitRule::Attr { from, .. } => from,
            EmitRule::Entity { from, .. } => from,
        };
        if seen.contains(&from.as_str()) {
            continue;
        }
        seen.push(from);
        if records.iter().any(|r| r.contains_key(from.as_str())) {
            hits += 1;
        }
    }
    Ok(hits)
}

pub fn sniff<'a>(shapes: &'a [Shape], stdout: &str) -> Result<Option<(&'a Shape, Records)>, Error> {
    for shape in shapes {
        let records = extractor(&shape.extract.id, &shape.extract.config)?.extract(stdout)?;
        if shape_hits(shape, &records)? >= shape.min_hits {
            return Ok(Some((shape, records)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv_config() -> ron::Value {
        ron::from_str::<ron::Value>("(sep: \":\")").unwrap()
    }

    #[test]
    fn known_ids_resolve_and_unknown_errors() {
        assert!(extractor("keyvalue", &kv_config()).is_ok());
        assert!(extractor("ndjson", &ron::Value::Unit).is_ok());
        assert!(matches!(
            extractor("nope", &ron::Value::Unit),
            Err(Error::UnknownExtractor { .. })
        ));
    }

    #[test]
    fn keyvalue_without_sep_is_a_bad_config() {
        assert!(matches!(
            extractor("keyvalue", &ron::Value::Unit),
            Err(Error::BadExtractConfig { .. })
        ));
    }
}
