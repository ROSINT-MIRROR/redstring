use crate::error::Error;
use crate::model::{EntityKind, ObsValue, RelationKind, url_host};

use super::extract::{Record, Records};
use super::spec::{AttrRule, EmitRule, LabelMode, ObsKind, Shape, kind_by_slug, rel_by_slug};


#[derive(Clone, Debug, PartialEq)]
pub enum Fact {

    Attr {
        key: String,
        value: String,
        kind: ObsKind,
    },


    Entity {
        kind: EntityKind,
        label: String,
        value: String,
        rel: RelationKind,
        outbound: bool,

        attrs: Vec<(String, String, ObsKind)>,
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
            EmitRule::Attr { from, key, kind } => {
                let Some(vals) = record.get(from) else { continue };
                let Some(first) = vals.first() else { continue };
                facts.push(Fact::Attr {
                    key: key.clone(),
                    value: first.clone(),
                    kind: *kind,
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
                let resolved = resolve_attrs(attrs, record)?;
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

fn resolve_attrs(
    rules: &[AttrRule],
    record: &Record,
) -> Result<Vec<(String, String, ObsKind)>, Error> {
    let mut out = Vec::new();
    for rule in rules {
        let Some(vals) = record.get(&rule.var) else { continue };
        let Some(first) = vals.first() else { continue };
        out.push((rule.key.clone(), first.clone(), rule.kind));
    }
    Ok(out)
}


pub fn parse_obs_value(kind: ObsKind, raw: &str) -> Result<ObsValue, Error> {
    Ok(match kind {
        ObsKind::Text => ObsValue::Text(raw.to_owned()),
        ObsKind::Number => number_or_text(raw),
        ObsKind::Date => date_or_text(raw),
        ObsKind::Coord => coord_or_text(raw),
        ObsKind::Bool => bool_or_text(raw),
    })
}

fn number_or_text(raw: &str) -> ObsValue {
    let cleaned = raw.trim().trim_start_matches('$').replace(',', "");
    let Ok(n) = cleaned.parse::<f64>() else {
        return ObsValue::Text(raw.to_owned());
    };
    ObsValue::Number(n)
}

fn bool_or_text(raw: &str) -> ObsValue {
    let t = raw.trim().to_ascii_lowercase();
    if matches!(t.as_str(), "true" | "yes" | "y" | "1") {
        return ObsValue::Bool(true);
    }
    if matches!(t.as_str(), "false" | "no" | "n" | "0") {
        return ObsValue::Bool(false);
    }
    ObsValue::Text(raw.to_owned())
}

fn coord_or_text(raw: &str) -> ObsValue {
    let parts: Vec<&str> = raw
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect();
    let [a, b] = parts.as_slice() else {
        return ObsValue::Text(raw.to_owned());
    };
    let (Ok(lat), Ok(lon)) = (a.parse::<f64>(), b.parse::<f64>()) else {
        return ObsValue::Text(raw.to_owned());
    };
    ObsValue::Coord { lat, lon }
}

fn date_or_text(raw: &str) -> ObsValue {
    let Some(secs) = parse_date(raw) else {
        return ObsValue::Text(raw.to_owned());
    };
    ObsValue::Date(secs)
}

fn parse_date(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let norm = s.replace(['/', '.'], "-");
    let mut halves = norm.splitn(2, |c: char| c == 'T' || c == 't' || c == ' ');
    let date_part = halves.next()?;
    let mut d = date_part.split('-');
    let y = to_i64(d.next()?)?;
    let mo = to_i64(d.next()?)?;
    let da = to_i64(d.next()?)?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&da) {
        return None;
    }
    let mut secs = days_from_civil(y, mo, da) * 86_400;
    let Some(time_part) = halves.next() else {
        return Some(secs);
    };
    secs += time_seconds(time_part);
    Some(secs)
}

fn time_seconds(part: &str) -> i64 {
    let (clock, offset) = offset_split(part.trim());
    let mut it = clock.split(':');
    let h = int_or_zero(it.next());
    let m = int_or_zero(it.next());
    let s = int_or_zero(it.next().and_then(|x| x.split('.').next()));
    h * 3_600 + m * 60 + s - offset
}

fn offset_split(t: &str) -> (String, i64) {
    let Some(stripped) = t.strip_suffix('Z') else {
        let Some(pos) = t.rfind(|c| c == '+' || c == '-') else {
            return (t.to_owned(), 0);
        };
        let (clock, off) = t.split_at(pos);
        let sign = if off.starts_with('-') { -1 } else { 1 };
        let mut oi = off[1..].split(':');
        let oh = int_or_zero(oi.next());
        let om = int_or_zero(oi.next());
        return (clock.to_owned(), sign * (oh * 3_600 + om * 60));
    };
    (stripped.to_owned(), 0)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn to_i64(s: &str) -> Option<i64> {
    let Ok(v) = s.trim().parse::<i64>() else {
        return None;
    };
    Some(v)
}

fn int_or_zero(s: Option<&str>) -> i64 {
    let Some(s) = s else {
        return 0;
    };
    let Ok(v) = s.trim().parse::<i64>() else {
        return 0;
    };
    v
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
                attrs: vec![AttrRule {
                    var: "datatype".into(),
                    key: "datatype".into(),
                    kind: ObsKind::Text,
                }],
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
            attrs: vec![("datatype".into(), "string".into(), ObsKind::Text)],
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Parameter,
            label: "age".into(),
            value: "age".into(),
            rel: RelationKind::AcceptsParam,
            outbound: true,
            attrs: vec![("datatype".into(), "int".into(), ObsKind::Text)],
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
                attrs: vec![AttrRule {
                    var: "registrar".into(),
                    key: "via_registrar".into(),
                    kind: ObsKind::Text,
                }],
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
            attrs: vec![("via_registrar".into(), "MarkMonitor".into(), ObsKind::Text)],
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Server,
            label: "NS2.EXAMPLE".into(),
            value: "NS2.EXAMPLE".into(),
            rel: RelationKind::ResolvesTo,
            outbound: true,
            attrs: vec![("via_registrar".into(), "MarkMonitor".into(), ObsKind::Text)],
        }));
    }

    #[test]
    fn parse_obs_value_types_by_kind_and_degrades_to_text() {
        assert_eq!(
            parse_obs_value(ObsKind::Text, "hello").unwrap(),
            ObsValue::Text("hello".into())
        );
        assert_eq!(
            parse_obs_value(ObsKind::Number, "180199672").unwrap(),
            ObsValue::Number(180199672.0)
        );
        assert_eq!(
            parse_obs_value(ObsKind::Number, "1,234").unwrap(),
            ObsValue::Number(1234.0),
            "thousands separators are stripped"
        );
        assert_eq!(
            parse_obs_value(ObsKind::Date, "2000-01-01T00:00:00Z").unwrap(),
            ObsValue::Date(946_684_800),
            "iso datetime with Z lands on the known epoch second"
        );
        assert_eq!(
            parse_obs_value(ObsKind::Date, "2000-01-01").unwrap(),
            ObsValue::Date(946_684_800),
            "a bare date is midnight UTC"
        );
        assert_eq!(
            parse_obs_value(ObsKind::Date, "2000-01-01T05:00:00+05:00").unwrap(),
            ObsValue::Date(946_684_800),
            "numeric offset is folded back to UTC"
        );
        assert_eq!(
            parse_obs_value(ObsKind::Coord, "40.5,-74.2").unwrap(),
            ObsValue::Coord { lat: 40.5, lon: -74.2 }
        );
        assert_eq!(
            parse_obs_value(ObsKind::Coord, "40.5 -74.2").unwrap(),
            ObsValue::Coord { lat: 40.5, lon: -74.2 },
            "space-separated coordinates parse too"
        );
        assert_eq!(parse_obs_value(ObsKind::Bool, "TRUE").unwrap(), ObsValue::Bool(true));
        assert_eq!(parse_obs_value(ObsKind::Bool, "no").unwrap(), ObsValue::Bool(false));

        assert_eq!(
            parse_obs_value(ObsKind::Date, "sometime last spring").unwrap(),
            ObsValue::Text("sometime last spring".into()),
            "an unparseable date degrades to the raw text, never dropped"
        );
        assert_eq!(
            parse_obs_value(ObsKind::Number, "N/A").unwrap(),
            ObsValue::Text("N/A".into())
        );
        assert_eq!(
            parse_obs_value(ObsKind::Coord, "1,2,3").unwrap(),
            ObsValue::Text("1,2,3".into()),
            "a three-part coordinate is not a lat,lon pair"
        );
    }

    #[test]
    fn attr_emit_carries_its_declared_kind() {
        let cfg = ron::from_str::<ron::Value>("(sep: \":\")").unwrap();
        let records = crate::modules::extract::extractor("keyvalue", &cfg)
            .unwrap()
            .extract("Registry Expiry Date: 2028-09-14T04:00:00Z\n")
            .unwrap();
        let shape = Shape {
            id: "icann".into(),
            extract: Extract {
                id: "keyvalue".into(),
                config: cfg,
            },
            min_hits: 1,
            emit: vec![EmitRule::Attr {
                from: "registry expiry date".into(),
                key: "expires".into(),
                kind: ObsKind::Date,
            }],
        };
        let facts = emit(&shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr {
            key: "expires".into(),
            value: "2028-09-14T04:00:00Z".into(),
            kind: ObsKind::Date,
        }));
    }
}
