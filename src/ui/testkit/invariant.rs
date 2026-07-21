use std::collections::HashSet;

use crate::error::Error;
use crate::model::{CaseFile, Origin, slugify};
use crate::project::{Project, read_case, validate};
use crate::ui::RedstringApp;

fn slug_is_derivable(slug: &str, label: &str, value: &str) -> Result<bool, Error> {
    if slug.is_empty() {
        return Ok(false);
    }
    let base = slugify(label)?;
    if slug == base {
        return Ok(true);
    }
    if !value.trim().is_empty() && slug == slugify(value)? {
        return Ok(true);
    }
    let prefix = format!("{base}_");
    let Some(rest) = slug.strip_prefix(&prefix) else {
        return Ok(false);
    };
    Ok(!rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

pub fn check_graph(app: &RedstringApp) -> Result<(), Error> {
    let g = app.g.g();
    let mut seen_slug: HashSet<String> = HashSet::new();
    let mut seen_value: HashSet<(usize, String)> = HashSet::new();
    for idx in g.node_indices() {
        let Some(n) = g.node_weight(idx) else {
            continue;
        };
        let e = n.payload();
        if e.slug.is_empty() {
            return Err(Error::InvariantViolated {
                what: format!("node {} labelled {:?} has an empty slug", idx.index(), e.label),
            });
        }
        if !seen_slug.insert(e.slug.clone()) {
            return Err(Error::InvariantViolated {
                what: format!("two nodes share slug '{}'", e.slug),
            });
        }
        if !slug_is_derivable(&e.slug, &e.label, &e.value)? {
            return Err(Error::InvariantViolated {
                what: format!(
                    "slug '{}' does not track label {:?} nor value {:?}",
                    e.slug, e.label, e.value
                ),
            });
        }
        if !e.value.trim().is_empty() {
            let key = (e.kind as usize, slugify(&e.value)?);
            if !seen_value.insert(key.clone()) {
                return Err(Error::InvariantViolated {
                    what: format!(
                        "two {:?} nodes share normalized value '{}' — duplicate entity",
                        e.kind, key.1
                    ),
                });
            }
        }
        for o in &e.observations {
            if o.id.0.is_empty() {
                return Err(Error::InvariantViolated {
                    what: format!("observation '{}' on '{}' is unstamped (empty id)", o.name, e.slug),
                });
            }
            if o.author.name.is_empty() {
                return Err(Error::InvariantViolated {
                    what: format!("observation '{}' on '{}' has no author", o.name, e.slug),
                });
            }
        }
    }
    for eidx in g.edge_indices() {
        let Some(ed) = g.edge_weight(eidx) else {
            continue;
        };
        let r = ed.payload();
        if matches!(r.origin, Origin::Lookup(_)) && r.sources.is_empty() {
            let ep = g.edge_endpoints(eidx).map(|(a, b)| (a.index(), b.index()));
            return Err(Error::InvariantViolated {
                what: format!("lookup-created {:?} edge {ep:?} carries zero sources", r.kind),
            });
        }
    }
    Ok(())
}

pub fn check_case_file(cf: &CaseFile) -> Result<(), Error> {
    let mut seen_slug: HashSet<&str> = HashSet::new();
    let mut seen_value: HashSet<(usize, String)> = HashSet::new();
    for e in &cf.data.entities {
        if e.slug.is_empty() {
            return Err(Error::InvariantViolated {
                what: format!("entity record labelled {:?} has an empty slug", e.label),
            });
        }
        if !seen_slug.insert(e.slug.as_str()) {
            return Err(Error::InvariantViolated {
                what: format!("two entity records share slug '{}'", e.slug),
            });
        }
        if !slug_is_derivable(&e.slug, &e.label, &e.value)? {
            return Err(Error::InvariantViolated {
                what: format!(
                    "record slug '{}' does not track label {:?} nor value {:?}",
                    e.slug, e.label, e.value
                ),
            });
        }
        if !e.value.trim().is_empty() {
            let key = (e.kind as usize, slugify(&e.value)?);
            if !seen_value.insert(key.clone()) {
                return Err(Error::InvariantViolated {
                    what: format!(
                        "two {:?} entity records share normalized value '{}'",
                        e.kind, key.1
                    ),
                });
            }
        }
        for o in &e.observations {
            if o.id.0.is_empty() {
                return Err(Error::InvariantViolated {
                    what: format!("record observation '{}' on '{}' is unstamped", o.name, e.slug),
                });
            }
            if o.author.name.is_empty() {
                return Err(Error::InvariantViolated {
                    what: format!("record observation '{}' on '{}' has no author", o.name, e.slug),
                });
            }
        }
    }
    let known: HashSet<&str> = cf.data.entities.iter().map(|e| e.slug.as_str()).collect();
    for r in &cf.data.relations {
        if !known.contains(r.from.as_str()) {
            return Err(Error::InvariantViolated {
                what: format!("relation cites unknown 'from' slug '{}'", r.from),
            });
        }
        if !known.contains(r.to.as_str()) {
            return Err(Error::InvariantViolated {
                what: format!("relation cites unknown 'to' slug '{}'", r.to),
            });
        }
        if matches!(r.origin, Origin::Lookup(_)) && r.sources.is_empty() {
            return Err(Error::InvariantViolated {
                what: format!("lookup-created {:?} relation {}->{} has zero sources", r.kind, r.from, r.to),
            });
        }
    }
    Ok(())
}

pub fn assert_round_trip(before: &CaseFile, after: &CaseFile) -> Result<(), Error> {
    let mut a = before.data.clone();
    let mut b = after.data.clone();
    a.entities.sort_by(|x, y| x.slug.cmp(&y.slug));
    b.entities.sort_by(|x, y| x.slug.cmp(&y.slug));
    a.relations
        .sort_by_key(|r| (r.from.clone(), r.to.clone(), r.kind as usize, r.created_at));
    b.relations
        .sort_by_key(|r| (r.from.clone(), r.to.clone(), r.kind as usize, r.created_at));
    if a.entities != b.entities {
        return Err(Error::InvariantViolated {
            what: "entities differ across save/load (beyond canonical slug ordering)".to_owned(),
        });
    }
    if a.relations != b.relations {
        return Err(Error::InvariantViolated {
            what: "relations differ across save/load".to_owned(),
        });
    }
    if before.display != after.display {
        return Err(Error::InvariantViolated {
            what: "display state differs across save/load".to_owned(),
        });
    }
    Ok(())
}

pub fn check_case_dir(project: &Project) -> Result<(), Error> {
    let findings = validate(project)?;
    if !findings.is_empty() {
        return Err(Error::InvariantViolated {
            what: format!(
                "--check reported {} finding(s): {}",
                findings.len(),
                findings.join(" | ")
            ),
        });
    }
    let cf = read_case(project)?;
    check_case_file(&cf)
}
