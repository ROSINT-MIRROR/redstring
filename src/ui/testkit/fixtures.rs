use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::project::Project;

pub fn module_ron(id: &str) -> Result<String, Error> {
    let text = match id {
        "whois" => include_str!("../../../modules/whois.ron"),
        "tlsx" => include_str!("../../../modules/tlsx.ron"),
        "crtsh" => include_str!("../../../modules/crtsh.ron"),
        "katana" => include_str!("../../../modules/katana.ron"),
        "dnsx" => include_str!("../../../modules/dnsx.ron"),
        "candel" => include_str!("../../../modules/candel.ron"),
        other => {
            return Err(Error::Testkit {
                what: format!("unknown builtin module '{other}'"),
            });
        }
    };
    Ok(text.to_owned())
}

pub const MODULE_IDS: [&str; 6] = ["whois", "tlsx", "crtsh", "katana", "dnsx", "candel"];

pub fn fixture(name: &str) -> Result<String, Error> {
    let text = match name {
        "whois-domain.txt" => include_str!("fixtures/whois-domain.txt"),
        "whois-ip.txt" => include_str!("fixtures/whois-ip.txt"),
        "tlsx.jsonl" => include_str!("fixtures/tlsx.jsonl"),
        "crtsh.jsonl" => include_str!("fixtures/crtsh.jsonl"),
        "crtsh-response.json" => include_str!("fixtures/crtsh-response.json"),
        "crtsh-404.html" => include_str!("fixtures/crtsh-404.html"),
        "katana.jsonl" => include_str!("fixtures/katana.jsonl"),
        "dnsx.jsonl" => include_str!("fixtures/dnsx.jsonl"),
        "candel.jsonl" => include_str!("fixtures/candel.jsonl"),
        other => {
            return Err(Error::Testkit {
                what: format!("unknown fixture '{other}'"),
            });
        }
    };
    Ok(text.to_owned())
}

pub fn sample_case_src() -> Result<PathBuf, Error> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ui/testkit/fixtures/sample_case"))
}

pub fn open_sample_case(into: &Path) -> Result<Project, Error> {
    copy_tree(&sample_case_src()?, into)?;
    Project::open(into)
}

pub fn open_timeline_case(into: &Path) -> Result<Project, Error> {
    copy_tree(&sample_case_src()?.with_file_name("timeline_case"), into)?;
    Project::open(into)
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(dst).map_err(|source| Error::Io {
        path: dst.to_path_buf(),
        source,
    })?;
    let entries = std::fs::read_dir(src).map_err(|source| Error::Io {
        path: src.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: src.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let Some(name) = path.file_name() else {
            continue;
        };
        let target = dst.join(name);
        if path.is_dir() {
            copy_tree(&path, &target)?;
        } else {
            std::fs::copy(&path, &target).map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
        }
    }
    Ok(())
}
