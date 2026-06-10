use crate::error::Error;
use crate::model::{EntityKind, RelationKind};

use super::extract::Vars;
use super::spec::{EmitRule, Shape, kind_by_slug, rel_by_slug};


#[derive(Clone, Debug, PartialEq)]
pub enum Fact {

    Attr {
        key: String,
        value: String,
    },


    Entity {
        kind: EntityKind,
        label: String,
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
            } => {
                let Some(vals) = vars.get(from) else { continue };
                let Some(k) = kind_by_slug(kind)? else { continue };
                let Some(r) = rel_by_slug(rel)? else { continue };
                let resolved = resolve_attrs(attrs, vars);
                for v in vals {
                    facts.push(Fact::Entity {
                        kind: k,
                        label: v.clone(),
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

fn resolve_attrs(rules: &[(String, String)], vars: &Vars) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (var, key) in rules {
        let Some(vals) = vars.get(var) else { continue };
        let Some(first) = vals.first() else { continue };
        out.push((key.clone(), first.clone()));
    }
    out
}
