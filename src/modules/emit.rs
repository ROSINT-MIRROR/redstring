use crate::error::Error;
use crate::model::{EntityKind, RelationKind, url_host};

use super::extract::Vars;
use super::spec::{EmitRule, LabelMode, Shape, kind_by_slug, rel_by_slug};


#[derive(Clone, Debug, PartialEq)]
pub enum Fact {

    Attr {
        key: String,
        value: String,
    },


    Entity {
        kind: EntityKind,
        label: String,
        value: String,
        rel: RelationKind,
        outbound: bool,

        attrs: Vec<(String, String)>,
    },
}


pub fn emit(shape: &Shape, vars: &Vars) -> Result<Vec<Fact>, Error> {
    let mut facts = Vec::new();
    for rule in &shape.emit {
        match rule {
            EmitRule::Attr { from, key } => {
                let Some(vals) = vars.get(from) else { continue };
                let Some(first) = vals.first() else { continue };
                facts.push(Fact::Attr {
                    key: key.clone(),
                    value: first.clone(),
                });
            }
            EmitRule::Entity {
                from,
                kind,
                rel,
                outbound,
                attrs,
                label,
            } => {
                let Some(vals) = vars.get(from) else { continue };
                let Some(k) = kind_by_slug(kind)? else { continue };
                let Some(r) = rel_by_slug(rel)? else { continue };
                let resolved = resolve_attrs(attrs, vars);
                for v in vals {
                    let display = display_label(label, v)?;
                    facts.push(Fact::Entity {
                        kind: k,
                        label: display,
                        value: v.clone(),
                        rel: r,
                        outbound: *outbound,
                        attrs: resolved.clone(),
                    });
                }
            }
        }
    }
    Ok(facts)
}

fn display_label(mode: &LabelMode, value: &str) -> Result<String, Error> {
    let LabelMode::Host = mode else {
        return Ok(value.to_owned());
    };
    let Some(host) = url_host(value)? else {
        return Ok(value.to_owned());
    };
    Ok(host)
}

fn resolve_attrs(rules: &[(String, String)], vars: &Vars) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (var, key) in rules {
        let Some(vals) = vars.get(var) else { continue };
        let Some(first) = vals.first() else { continue };
        out.push((key.clone(), first.clone()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn host_shape() -> Shape {
        Shape {
            id: "candel".into(),
            extract: crate::modules::spec::Extract::Ndjson,
            min_hits: 1,
            emit: vec![EmitRule::Entity {
                from: "url".into(),
                kind: "website".into(),
                rel: "uses_account".into(),
                outbound: true,
                attrs: Vec::new(),
                label: LabelMode::Host,
            }],
        }
    }

    #[test]
    fn host_mode_labels_by_domain_keeps_url_as_value() {
        let mut vars: Vars = HashMap::new();
        vars.insert(
            "url".into(),
            vec![
                "https://github.com/torvalds".into(),
                "https://www.reddit.com/user/torvalds".into(),
            ],
        );
        let facts = emit(&host_shape(), &vars).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Website,
            label: "github.com".into(),
            value: "https://github.com/torvalds".into(),
            rel: RelationKind::UsesAccount,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Website,
            label: "reddit.com".into(),
            value: "https://www.reddit.com/user/torvalds".into(),
            rel: RelationKind::UsesAccount,
            outbound: true,
            attrs: Vec::new(),
        }));
    }
}
