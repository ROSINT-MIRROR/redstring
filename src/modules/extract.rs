use std::collections::HashMap;

use crate::error::Error;

use super::spec::{EmitRule, Extract, Shape};


pub type Vars = HashMap<String, Vec<String>>;


pub fn extract(ex: &Extract, stdout: &str) -> Result<Vars, Error> {
    match ex {
        Extract::KeyValue { sep, comments } => Ok(key_value(sep, comments, stdout)),
        Extract::Ndjson => Ok(ndjson(stdout)),
    }
}


fn ndjson(stdout: &str) -> Vars {
    let mut vars: Vars = HashMap::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(serde_json::Value::Object(obj)) = serde_json::from_str::<serde_json::Value>(line)
        else {
            continue;
        };
        for (k, v) in obj {
            let serde_json::Value::String(val) = v else {
                continue;
            };
            let key = k.trim().to_lowercase();
            let val = val.trim();
            if key.is_empty() || val.is_empty() {
                continue;
            }
            let entry = vars.entry(key).or_default();
            if !entry.iter().any(|e| e == val) {
                entry.push(val.to_owned());
            }
        }
    }
    vars
}


fn key_value(sep: &str, comments: &str, stdout: &str) -> Vars {
    let mut vars: Vars = HashMap::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !comments.is_empty() && line.starts_with(comments) {
            continue;
        }
        let Some((k, v)) = line.split_once(sep) else {
            continue;
        };
        let key = k.trim().to_lowercase();
        let val = v.trim();
        if key.is_empty() || val.is_empty() {
            continue;
        }
        let entry = vars.entry(key).or_default();
        if !entry.iter().any(|e| e == val) {
            entry.push(val.to_owned());
        }
    }
    vars
}


pub fn shape_hits(shape: &Shape, vars: &Vars) -> Result<usize, Error> {
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
        if vars.contains_key(from.as_str()) {
            hits += 1;
        }
    }
    Ok(hits)
}


pub fn sniff<'a>(shapes: &'a [Shape], stdout: &str) -> Result<Option<(&'a Shape, Vars)>, Error> {
    for shape in shapes {
        let vars = extract(&shape.extract, stdout)?;
        if shape_hits(shape, &vars)? >= shape.min_hits {
            return Ok(Some((shape, vars)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_value_trims_skips_and_collects() {
        let text = "\
% comment line
   Registrar: MarkMonitor Inc.
   Name Server: NS1.GOOGLE.COM
   Name Server: NS2.GOOGLE.COM
   Name Server: NS1.GOOGLE.COM
   Registrar URL: http://www.markmonitor.com
   OriginAS:
no separator here
";
        let vars = key_value(":", "%", text);
        assert_eq!(vars["registrar"], vec!["MarkMonitor Inc."]);
        assert_eq!(vars["name server"], vec!["NS1.GOOGLE.COM", "NS2.GOOGLE.COM"]);
        assert_eq!(vars["registrar url"], vec!["http://www.markmonitor.com"]);
        assert!(!vars.contains_key("originas"), "empty values dropped");
        assert!(!vars.contains_key("% comment line"));
    }

    #[test]
    fn first_separator_wins() {
        let vars = key_value(":", "", "Ref: https://rdap.arin.net/registry/ip/8.8.8.0");
        assert_eq!(vars["ref"], vec!["https://rdap.arin.net/registry/ip/8.8.8.0"]);
    }

    #[test]
    fn ndjson_collects_fields_dedups_and_skips_junk() {
        let text = "\
{\"slug\":\"github\",\"url\":\"https://github.com/torvalds\"}
not json at all
{\"slug\":\"gitlab\",\"url\":\"https://gitlab.com/torvalds\"}
{\"slug\":\"github\",\"url\":\"https://github.com/torvalds\"}
{\"slug\":\"empty\",\"url\":\"\"}
{\"count\":3}
";
        let vars = ndjson(text);
        assert_eq!(
            vars["url"],
            vec!["https://github.com/torvalds", "https://gitlab.com/torvalds"]
        );
        assert_eq!(vars["slug"], vec!["github", "gitlab", "empty"]);
        assert!(!vars.contains_key("count"), "non-string values dropped");
    }
}
