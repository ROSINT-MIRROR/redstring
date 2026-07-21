use serde::Deserialize;

use crate::error::Error;
use crate::model::{EntityKind, KIND_META, RELATION_META, RelationKind, slugify};

use super::extract::extractor;


#[derive(Clone, Debug, Deserialize)]
pub struct ModuleSpec {
    pub id: String,
    pub label: String,

    pub applies_to: Vec<String>,

    pub bin: String,

    #[serde(default)]
    pub requires: Vec<String>,

    pub args: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    #[serde(default)]
    pub keys: Vec<String>,

    #[serde(default)]
    pub headers: Vec<(String, String)>,

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
pub struct Extract {
    pub id: String,
    #[serde(default = "default_config")]
    pub config: ron::Value,
}

fn default_config() -> ron::Value {
    ron::Value::Unit
}


#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum ObsKind {
    #[default]
    Text,
    Number,
    Date,
    Coord,
    Bool,
}


#[derive(Clone, Debug)]
pub struct AttrRule {
    pub var: String,
    pub key: String,
    pub kind: ObsKind,
}

impl<'de> serde::Deserialize<'de> for AttrRule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AttrVisitor;
        impl<'de> serde::de::Visitor<'de> for AttrVisitor {
            type Value = AttrRule;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a (var, key) or (var, key, kind) attribute tuple")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let Some(var) = seq.next_element::<String>()? else {
                    return Err(serde::de::Error::invalid_length(0, &self));
                };
                let Some(key) = seq.next_element::<String>()? else {
                    return Err(serde::de::Error::invalid_length(1, &self));
                };
                let Some(kind) = seq.next_element::<ObsKind>()? else {
                    return Ok(AttrRule { var, key, kind: ObsKind::Text });
                };
                Ok(AttrRule { var, key, kind })
            }
        }
        deserializer.deserialize_tuple(3, AttrVisitor)
    }
}


#[derive(Clone, Debug, Deserialize)]
pub enum EmitRule {
    Attr {
        from: String,
        key: String,
        #[serde(default)]
        kind: ObsKind,
    },
    Entity {
        from: String,
        kind: String,
        rel: String,
        #[serde(default = "default_outbound")]
        outbound: bool,

        #[serde(default)]
        attrs: Vec<AttrRule>,

        #[serde(default)]
        label: LabelMode,
    },
}


#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub enum LabelMode {
    #[default]
    Verbatim,

    Host,
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
            if let Err(err) = extractor(&shape.extract.id, &shape.extract.config) {
                problems.push(format!("shape '{}': {err}", shape.id));
            }
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
        assert_eq!(kind_by_slug("organization").unwrap(), Some(EntityKind::Organization));
        assert_eq!(kind_by_slug("shell_company").unwrap(), None);
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
            requires: Vec::new(),
            args: vec!["{label}".into(), "--slug={slug}".into(), "{attr:registrar}".into()],
            timeout_secs: 5,
            keys: Vec::new(),
            headers: Vec::new(),
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

    #[test]
    fn attr_rules_accept_two_and_three_tuples() {
        let rules: Vec<AttrRule> =
            ron::from_str(r#"[("country", "country_code"), ("expires", "expiry", Date)]"#).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].var, "country");
        assert_eq!(rules[0].key, "country_code");
        assert_eq!(rules[0].kind, ObsKind::Text, "an omitted kind defaults to Text");
        assert_eq!(rules[1].var, "expires");
        assert_eq!(rules[1].key, "expiry");
        assert_eq!(rules[1].kind, ObsKind::Date);
    }

    #[test]
    fn attr_emit_rule_kind_defaults_to_text() {
        let untyped: EmitRule = ron::from_str(r#"Attr(from: "x", key: "y")"#).unwrap();
        let EmitRule::Attr { kind, .. } = untyped else {
            panic!("expected an Attr rule");
        };
        assert_eq!(kind, ObsKind::Text);
        let typed: EmitRule = ron::from_str(r#"Attr(from: "x", key: "y", kind: Number)"#).unwrap();
        let EmitRule::Attr { kind, .. } = typed else {
            panic!("expected an Attr rule");
        };
        assert_eq!(kind, ObsKind::Number);
    }
}
