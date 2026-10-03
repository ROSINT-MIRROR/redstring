use serde::Deserialize;

use crate::error::Error;
use crate::model::{ArchiveRecord, ArchiveStatus, ArchiveVia, civil_ymd, days_from_civil};

use super::config::DETAIL_MAX;

#[derive(Deserialize)]
struct WireLine {
    witness: String,
    url: String,
    status: String,
    reason: String,
    snapshot: Option<String>,
    ts: Option<String>,
    detail: Option<String>,
}

pub struct Asked<'a> {
    pub url: &'a str,
    pub via: ArchiveVia,
    pub now: i64,
}

pub fn parse_line(line: &str, asked: &Asked) -> Result<ArchiveRecord, Error> {
    let wire: WireLine = serde_json::from_str(line).map_err(|source| broken(format!("not a witness line ({source}): {}", clip(line, 160))))?;
    if wire.url != asked.url {
        return Err(broken(format!("line answers for {} — asked about {}", wire.url, asked.url)));
    }
    let status = ArchiveStatus::from_word(&wire.status)?;
    Ok(ArchiveRecord {
        url: wire.url,
        witness: code(wire.witness, "witness")?,
        status,
        reason: code(wire.reason, "reason")?,
        snapshot: snapshot_of(status, wire.snapshot)?,
        archived_at: wire.ts.as_deref().map(ts14_secs).transpose()?,
        detail: wire.detail.map(|text| clip(&text, DETAIL_MAX)),
        via: asked.via,
        checked_at: asked.now,
    })
}

pub fn is_web_url(url: &str) -> Result<bool, Error> {
    let scheme = url.starts_with("https://") || url.starts_with("http://");
    Ok(scheme && !url.chars().any(|c| c.is_whitespace() || c.is_control()))
}

pub fn ts14_secs(ts: &str) -> Result<i64, Error> {
    if ts.len() != 14 || !ts.bytes().all(|b| b.is_ascii_digit()) {
        return Err(broken(format!("ts '{ts}' is not ts14 (YYYYMMDDhhmmss)")));
    }
    let field = |from: usize, to: usize| -> Result<i64, Error> {
        ts[from..to].parse::<i64>().map_err(|source| broken(format!("ts '{ts}': {source}")))
    };
    let (year, month, day) = (field(0, 4)?, field(4, 6)?, field(6, 8)?);
    let (hour, minute, second) = (field(8, 10)?, field(10, 12)?, field(12, 14)?);
    let days = days_from_civil(year, month, day)?;
    if civil_ymd(days * 86_400)? != (year, month, day) || hour > 23 || minute > 59 || second > 59 {
        return Err(broken(format!("ts '{ts}' is not a real UTC instant")));
    }
    Ok(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn snapshot_of(status: ArchiveStatus, raw: Option<String>) -> Result<Option<String>, Error> {
    let Some(url) = raw else {
        if status == ArchiveStatus::Confirmed {
            return Err(broken("a confirmed line carries no snapshot url".to_owned()));
        }
        return Ok(None);
    };
    if !is_web_url(&url)? {
        return Err(broken(format!("snapshot '{}' is not an http(s) url", clip(&url, 160))));
    }
    Ok(Some(url))
}

fn code(word: String, what: &str) -> Result<String, Error> {
    let lawful = !word.is_empty() && word.len() <= 32 && word.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if !lawful {
        return Err(broken(format!("{what} '{}' is not a machine code", clip(&word, 40))));
    }
    Ok(word)
}

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn broken(what: String) -> Error {
    Error::Alibi {
        what: format!("output broke the documented --json contract: {what}"),
    }
}
