use crate::error::Error;
use crate::model::{DatePrecision, DateVal, EntityKind, ObsValue, RelationKind, url_host};

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
    let Some((secs, precision)) = parse_date(raw) else {
        return ObsValue::Text(raw.to_owned());
    };
    ObsValue::Date(DateVal { secs, precision })
}

fn parse_date(raw: &str) -> Option<(i64, DatePrecision)> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let Some(rest) = strip_circa(s) else {
        return parse_fuzzy_or_ymd(s);
    };
    let (secs, _) = parse_ymd(rest)?;
    Some((secs, DatePrecision::Circa))
}

fn parse_fuzzy_or_ymd(s: &str) -> Option<(i64, DatePrecision)> {
    let Some(secs) = parse_century(s) else {
        let Some(secs) = parse_decade(s) else {
            return parse_ymd(s);
        };
        return Some((secs, DatePrecision::Decade));
    };
    Some((secs, DatePrecision::Century))
}

fn strip_circa(s: &str) -> Option<&str> {
    let lower = s.to_ascii_lowercase();
    for pre in ["~", "circa", "ca.", "c."] {
        if !lower.starts_with(pre) {
            continue;
        }
        let rest = s[pre.len()..].trim_start();
        if pre == "~" || rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_digit()) {
            return Some(rest);
        }
    }
    None
}

fn parse_century(s: &str) -> Option<i64> {
    let lower = s.trim().to_ascii_lowercase();
    if !(lower.ends_with("century") || lower.ends_with("c.")) {
        return None;
    }
    let digits: String = lower.chars().take_while(|c| c.is_ascii_digit()).collect();
    let n = to_i64(&digits)?;
    if !(1..=99).contains(&n) {
        return None;
    }
    civil_secs((n - 1) * 100 + 1, 1, 1)
}

fn parse_decade(s: &str) -> Option<i64> {
    let Some(head) = s.trim().strip_suffix('s') else {
        return None;
    };
    let n = to_i64(head)?;
    if n % 10 != 0 || !(0..=9999).contains(&n) {
        return None;
    }
    civil_secs(n, 1, 1)
}

fn parse_ymd(raw: &str) -> Option<(i64, DatePrecision)> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let norm = s.replace(['/', '.'], "-");
    let mut halves = norm.splitn(2, |c: char| c == 'T' || c == 't' || c == ' ');
    let date_part = halves.next()?;
    let time_part = halves.next();
    let comps: Vec<&str> = date_part.split('-').collect();
    match comps.as_slice() {
        [y] => {
            if time_part.is_some() {
                return None;
            }
            Some((civil_secs(year_in_range(y)?, 1, 1)?, DatePrecision::Year))
        }
        [y, mo] => {
            if time_part.is_some() {
                return None;
            }
            let month = to_i64(mo)?;
            if !(1..=12).contains(&month) {
                return None;
            }
            Some((civil_secs(year_in_range(y)?, month, 1)?, DatePrecision::Month))
        }
        [y, mo, da] => {
            let month = to_i64(mo)?;
            let day = to_i64(da)?;
            if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
                return None;
            }
            let secs = civil_secs(to_i64(y)?, month, day)?;
            let Some(time_part) = time_part else {
                return Some((secs, DatePrecision::Day));
            };
            Some((secs + time_seconds(time_part), DatePrecision::Day))
        }
        _other => None,
    }
}

fn year_in_range(s: &str) -> Option<i64> {
    let n = to_i64(s)?;
    if !(-9999..=9999).contains(&n) {
        return None;
    }
    Some(n)
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

fn civil_secs(y: i64, m: i64, d: i64) -> Option<i64> {
    let Ok(days) = crate::model::days_from_civil(y, m, d) else {
        return None;
    };
    Some(days * 86_400)
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
            ObsValue::Date(DateVal { secs: 946_684_800, precision: DatePrecision::Day }),
            "iso datetime with Z lands on the known epoch second"
        );
        assert_eq!(
            parse_obs_value(ObsKind::Date, "2000-01-01").unwrap(),
            ObsValue::Date(DateVal { secs: 946_684_800, precision: DatePrecision::Day }),
            "a bare date is midnight UTC"
        );
        assert_eq!(
            parse_obs_value(ObsKind::Date, "2000-01-01T05:00:00+05:00").unwrap(),
            ObsValue::Date(DateVal { secs: 946_684_800, precision: DatePrecision::Day }),
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

    fn parse_date_str(raw: &str) -> ObsValue {
        parse_obs_value(ObsKind::Date, raw).unwrap()
    }

    #[test]
    fn date_parse_detects_precision_from_input_shape() {
        assert_eq!(
            parse_date_str("2012"),
            ObsValue::Date(DateVal { secs: 1_325_376_000, precision: DatePrecision::Year }),
            "a bare year keeps year precision anchored to Jan 1"
        );
        assert_eq!(
            parse_date_str("2012-11"),
            ObsValue::Date(DateVal { secs: 1_351_728_000, precision: DatePrecision::Month }),
            "year-month keeps month precision anchored to the first"
        );
        let ObsValue::Date(day) = parse_date_str("2019-04-12") else {
            panic!("a full date must stay a Date");
        };
        assert_eq!(day.precision, DatePrecision::Day);

        assert_eq!(parse_date_str("2012").text().unwrap(), "2012");
        assert_eq!(parse_date_str("2012-11").text().unwrap(), "2012-11");
        assert_eq!(parse_date_str("2019-04-12").text().unwrap(), "2019-04-12");
        assert_eq!(parse_date_str("~1976").text().unwrap(), "~1976");
        assert_eq!(parse_date_str("c. 1976").text().unwrap(), "~1976");
        assert_eq!(parse_date_str("circa 1976").text().unwrap(), "~1976");
        assert_eq!(parse_date_str("14th century").text().unwrap(), "14th c.");
        assert_eq!(parse_date_str("14th c.").text().unwrap(), "14th c.");
        assert_eq!(parse_date_str("2010s").text().unwrap(), "2010s");
    }

    #[test]
    fn ambiguous_or_ranged_dates_degrade_to_text_not_a_false_instant() {
        assert_eq!(
            parse_date_str("2012-2013"),
            ObsValue::Text("2012-2013".into()),
            "a year range is not one instant"
        );
        assert_eq!(
            parse_date_str("600 years"),
            ObsValue::Text("600 years".into()),
            "a duration is not a date"
        );
        assert_eq!(
            parse_date_str("November 2014"),
            ObsValue::Text("November 2014".into()),
            "a month name we do not parse keeps its literal fidelity"
        );
        let ObsValue::Date(pre) = parse_date_str("1969-07-20") else {
            panic!("a pre-1970 date must still parse to a Date");
        };
        assert!(pre.secs < 0, "pre-epoch dates carry negative seconds: {}", pre.secs);
        assert_eq!(parse_date_str("1969-07-20").text().unwrap(), "1969-07-20");
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
