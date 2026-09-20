use std::collections::BTreeSet;
use std::path::Path;

use crate::error::Error;
use crate::model::CaseFile;

const MIN_TOKEN_LEN: usize = 3;

pub fn sealed_tokens(cf: &CaseFile, hidden: &BTreeSet<String>) -> Result<Vec<String>, Error> {
    let mut out: Vec<String> = Vec::new();
    for e in &cf.data.entities {
        if !hidden.contains(&e.slug) {
            continue;
        }
        out.push(e.slug.clone());
        if !e.label.trim().is_empty() {
            out.push(e.label.clone());
        }
        if !e.value.trim().is_empty() {
            out.push(e.value.clone());
        }
        for o in &e.observations {
            for s in &o.sources {
                if !s.reference.trim().is_empty() {
                    out.push(s.reference.clone());
                }
            }
        }
    }
    out.retain(|t| t.len() >= MIN_TOKEN_LEN);
    out.sort();
    out.dedup();
    Ok(out)
}

pub fn assert_clean(staging: &Path, tokens: &[String], files: &[String]) -> Result<(), Error> {
    if tokens.is_empty() {
        return Ok(());
    }
    for rel in files {
        for token in tokens {
            if rel.contains(token.as_str()) {
                return Err(Error::SealedLeak {
                    path: rel.clone(),
                    token: token.clone(),
                });
            }
        }
        let abs = staging.join(rel);
        let bytes = std::fs::read(&abs).map_err(|source| Error::Io { path: abs, source })?;
        for token in tokens {
            if contains_bytes(&bytes, token.as_bytes()) {
                return Err(Error::SealedLeak {
                    path: rel.clone(),
                    token: token.clone(),
                });
            }
        }
    }
    Ok(())
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}
