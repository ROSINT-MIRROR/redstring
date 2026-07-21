use std::path::PathBuf;

use crate::error::Error;
use crate::modules::emit::{Fact, emit};
use crate::modules::extract::sniff;
use crate::modules::spec::ModuleSpec;

pub fn parse(spec_ron: &str, raw: &str) -> Result<(String, Vec<Fact>), Error> {
    let spec: ModuleSpec = ron::from_str(spec_ron).map_err(|source| Error::RonParse {
        path: PathBuf::from("<testkit module spec>"),
        source,
    })?;
    let Some((shape, records)) = sniff(&spec.shapes, raw)? else {
        return Ok((String::new(), Vec::new()));
    };
    let shape_id = shape.id.clone();
    let facts = emit(shape, &records)?;
    Ok((shape_id, facts))
}
