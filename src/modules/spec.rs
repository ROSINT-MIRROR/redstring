use serde::Deserialize;

use crate::error::Error;
use crate::model::{EntityKind, KIND_META, RELATION_META, RelationKind, slugify};


#[derive(Clone, Debug, Deserialize)]
pub struct ModuleSpec {
    pub id: String,
    pub label: String,

    pub applies_to: Vec<String>,

    pub bin: String,

    pub args: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    #[serde(default)]
    pub keys: Vec<String>,

    pub shapes: Vec<Shape>,
}


#[derive(Clone, Debug, Deserialize)]
pub struct Shape {
    pub id: String,
    pub extract: Extract,
    #[serde(default = "default_min_hits")]
    pub min_hits: usize,
    pub emit: Vec<EmitRule>,
}


#[derive(Clone, Debug, Deserialize)]
pub enum Extract {
    KeyValue {
        sep: String,
        #[serde(default)]
        comments: String,
    },
}


#[derive(Clone, Debug, Deserialize)]
pub enum EmitRule {
    Attr {
        from: String,
        key: String,
    },
    Entity {
        from: String,
        kind: String,
        rel: String,
        #[serde(default = "default_outbound")]
        outbound: bool,

        #[serde(default)]
        attrs: Vec<(String, String)>,
    },
}

fn default_timeout() -> u64 {
    20
}

fn default_min_hits() -> usize {
    3
}

fn default_outbound() -> bool {
    true
}


pub fn kind_by_slug(slug: &str) -> Result<Option<EntityKind>, Error> {
    for m in &KIND_META {
        if slugify(m.label)? == slug {
            return Ok(Some(m.kind));
        }
    }
    Ok(None)
}


pub fn rel_by_slug(slug: &str) -> Result<Option<RelationKind>, Error> {
    for m in &RELATION_META {
        if slugify(m.label)? == slug {
            return Ok(Some(m.kind));
        }
    }
    Ok(None)
}

impl ModuleSpec {

    pub fn validate(&self) -> Result<Vec<String>, Error> {
        let mut problems = Vec::new();
        for k in &self.applies_to {
            if kind_by_slug(k)?.is_none() {
                problems.push(format!("unknown entity kind '{k}'"));
            }
        }
        for shape in &self.shapes {
            for rule in &shape.emit {
                let EmitRule::Entity { kind, rel, .. } = rule else {
                    continue;
                };
                if kind_by_slug(kind)?.is_none() {
                    problems.push(format!("shape '{}': unknown entity kind '{kind}'", shape.id));
                }
                if rel_by_slug(rel)?.is_none() {
                    problems.push(format!("shape '{}': unknown relation '{rel}'", shape.id));
                }
            }
        }
        Ok(problems)
    }

    pub fn kinds(&self) -> Result<Vec<EntityKind>, Error> {
        let mut out = Vec::new();
        for s in &self.applies_to {
            let Some(k) = kind_by_slug(s)? else { continue };
            out.push(k);
        }
        Ok(out)
    }

    pub fn render_args(
        &self,
        value: &str,
        label: &str,
        slug: &str,
        attrs: &[(String, String)],
    ) -> Result<Vec<String>, Error> {
        let mut out = Vec::new();
        for a in &self.args {
            out.push(render_arg(a, value, label, slug, attrs));
        }
        Ok(out)
    }
}

fn render_arg(
    template: &str,
    value: &str,
    label: &str,
    slug: &str,
    attrs: &[(String, String)],
) -> String {
    let mut s = template
        .replace("{value}", value)
        .replace("{label}", label)
        .replace("{slug}", slug);
    for (k, v) in attrs {
        s = s.replace(&format!("{{attr:{k}}}"), v);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_lookups_resolve_ontology() {
        assert_eq!(kind_by_slug("domain").unwrap(), Some(EntityKind::Domain));
        assert_eq!(kind_by_slug("ip_address").unwrap(), Some(EntityKind::IpAddress));
        assert_eq!(kind_by_slug("shell_company").unwrap(), Some(EntityKind::ShellCompany));
        assert_eq!(kind_by_slug("nope").unwrap(), None);
        assert_eq!(rel_by_slug("registered_at").unwrap(), Some(RelationKind::RegisteredAt));
        assert_eq!(rel_by_slug("resolves_to").unwrap(), Some(RelationKind::ResolvesTo));
        assert_eq!(rel_by_slug("related_to").unwrap(), Some(RelationKind::RelatedTo));
        assert_eq!(rel_by_slug("nope").unwrap(), None);
    }

    #[test]
    fn arg_templates_substitute() {
        let spec = ModuleSpec {
            id: "t".into(),
            label: "t".into(),
            applies_to: vec!["domain".into()],
            bin: "true".into(),
            args: vec!["{label}".into(), "--slug={slug}".into(), "{attr:registrar}".into()],
            timeout_secs: 5,
            keys: Vec::new(),
            shapes: Vec::new(),
        };
        let args = spec
            .render_args("93.184.216.34", "evil.example", "evil_example", &[("registrar".into(), "MarkMonitor".into())])
            .unwrap();
        assert_eq!(args, vec!["evil.example", "--slug=evil_example", "MarkMonitor"]);
        let spec2 = ModuleSpec { args: vec!["{value}".into()], ..spec };
        let args = spec2
            .render_args("93.184.216.34", "evil.example", "evil_example", &[])
            .unwrap();
        assert_eq!(args, vec!["93.184.216.34"]);
    }
}
