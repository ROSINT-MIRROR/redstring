use crate::error::Error;
use crate::model::{EntityKind, RelationKind, url_host};

use super::extract::{Record, Records};
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


pub fn emit(shape: &Shape, records: &Records) -> Result<Vec<Fact>, Error> {
    let mut facts = Vec::new();
    for record in records {
        emit_record(shape, record, &mut facts)?;
    }
    Ok(facts)
}

fn emit_record(shape: &Shape, record: &Record, facts: &mut Vec<Fact>) -> Result<(), Error> {
    for rule in &shape.emit {
        match rule {
            EmitRule::Attr { from, key } => {
                let Some(vals) = record.get(from) else { continue };
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
                let Some(vals) = record.get(from) else { continue };
                let Some(k) = kind_by_slug(kind)? else { continue };
                let Some(r) = rel_by_slug(rel)? else { continue };
                let resolved = resolve_attrs(attrs, record);
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
    Ok(())
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

fn resolve_attrs(rules: &[(String, String)], record: &Record) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (var, key) in rules {
        let Some(vals) = record.get(var) else { continue };
        let Some(first) = vals.first() else { continue };
        out.push((key.clone(), first.clone()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::spec::Extract;

    fn host_shape() -> Shape {
        Shape {
            id: "candel".into(),
            extract: Extract {
                id: "ndjson".into(),
                config: ron::Value::Unit,
            },
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

    fn record(pairs: &[(&str, &str)]) -> Record {
        let mut r = Record::new();
        for (k, v) in pairs {
            r.entry((*k).to_owned()).or_default().push((*v).to_owned());
        }
        r
    }

    #[test]
    fn host_mode_labels_by_domain_keeps_url_as_value() {
        let records = vec![record(&[
            ("url", "https://github.com/torvalds"),
            ("url", "https://www.reddit.com/user/torvalds"),
        ])];
        let facts = emit(&host_shape(), &records).unwrap();
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

    #[test]
    fn ndjson_records_keep_attrs_correlated_per_entity() {
        let text = "{\"name\":\"user\",\"datatype\":\"string\"}\n{\"name\":\"age\",\"datatype\":\"int\"}\n";
        let records = crate::modules::extract::extractor("ndjson", &ron::Value::Unit)
            .unwrap()
            .extract(text)
            .unwrap();
        assert_eq!(records.len(), 2, "one record per ndjson object");
        let shape = Shape {
            id: "params".into(),
            extract: Extract {
                id: "ndjson".into(),
                config: ron::Value::Unit,
            },
            min_hits: 1,
            emit: vec![EmitRule::Entity {
                from: "name".into(),
                kind: "parameter".into(),
                rel: "accepts_parameter".into(),
                outbound: true,
                attrs: vec![("datatype".into(), "datatype".into())],
                label: LabelMode::Verbatim,
            }],
        };
        let facts = emit(&shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Parameter,
            label: "user".into(),
            value: "user".into(),
            rel: RelationKind::AcceptsParam,
            outbound: true,
            attrs: vec![("datatype".into(), "string".into())],
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Parameter,
            label: "age".into(),
            value: "age".into(),
            rel: RelationKind::AcceptsParam,
            outbound: true,
            attrs: vec![("datatype".into(), "int".into())],
        }));
    }

    #[test]
    fn keyvalue_multivalue_from_fans_out_carrying_shared_attrs() {
        let cfg = ron::from_str::<ron::Value>("(sep: \":\")").unwrap();
        let text = "Registrar: MarkMonitor\nName Server: NS1.EXAMPLE\nName Server: NS2.EXAMPLE\n";
        let records = crate::modules::extract::extractor("keyvalue", &cfg)
            .unwrap()
            .extract(text)
            .unwrap();
        assert_eq!(records.len(), 1, "keyvalue yields exactly one record");
        let shape = Shape {
            id: "ns".into(),
            extract: Extract {
                id: "keyvalue".into(),
                config: cfg,
            },
            min_hits: 1,
            emit: vec![EmitRule::Entity {
                from: "name server".into(),
                kind: "server".into(),
                rel: "resolves_to".into(),
                outbound: true,
                attrs: vec![("registrar".into(), "via_registrar".into())],
                label: LabelMode::Verbatim,
            }],
        };
        let facts = emit(&shape, &records).unwrap();
        let servers = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Server, .. }))
            .count();
        assert_eq!(servers, 2, "one server per name server value");
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Server,
            label: "NS1.EXAMPLE".into(),
            value: "NS1.EXAMPLE".into(),
            rel: RelationKind::ResolvesTo,
            outbound: true,
            attrs: vec![("via_registrar".into(), "MarkMonitor".into())],
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Server,
            label: "NS2.EXAMPLE".into(),
            value: "NS2.EXAMPLE".into(),
            rel: RelationKind::ResolvesTo,
            outbound: true,
            attrs: vec![("via_registrar".into(), "MarkMonitor".into())],
        }));
    }
}
