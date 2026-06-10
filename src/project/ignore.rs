






use std::path::Path;

use crate::error::{Error, RsResult};

pub const IGNORE_FILE: &str = ".rsignore";

pub struct RsIgnore {
    patterns: Vec<String>,
}

impl RsIgnore {
    
    pub fn load(root: &Path) -> RsResult<Self> {
        let path = root.join(IGNORE_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                
                String::new()
            }
            Err(source) => return Err(Error::Io { path: path.to_path_buf(), source }),
        };
        let patterns = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_owned)
            .collect();
        Ok(Self { patterns })
    }

    
    pub fn matches(&self, rel: &str) -> RsResult<bool> {
        Ok(self.patterns.iter().any(|p| pattern_matches(p, rel)))
    }
}

fn pattern_matches(pat: &str, rel: &str) -> bool {
    let Some(dir) = pat.strip_suffix('/') else {
        return glob_match(pat, rel);
    };
    
    glob_match(dir, rel) || glob_match(&format!("{dir}/*"), rel)
}


fn glob_match(pat: &str, text: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == t[ti]) && p[pi] != '*' {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = pi;
            mark = ti;
            pi += 1;
        } else if star != usize::MAX {
            pi = star + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_semantics() {
        assert!(glob_match("extra/raw-*.mp4", "extra/raw-interview.mp4"));
        assert!(!glob_match("extra/raw-*.mp4", "extra/raw-interview.mp3"));
        assert!(glob_match("*", "anything/at/all"));
        assert!(glob_match("evidence/*/big.bin", "evidence/docs/big.bin"));
        assert!(!glob_match("raw-*.mp4", "extra/raw-x.mp4")); 
        assert!(glob_match("a*b*c", "aXbYc"));
        assert!(!glob_match("a*b*c", "aXbY"));
    }

    #[test]
    fn dir_patterns_cover_subtrees() {
        assert!(pattern_matches("extra/unsorted/", "extra/unsorted"));
        assert!(pattern_matches("extra/unsorted/", "extra/unsorted/deep/file.jpg"));
        assert!(!pattern_matches("extra/unsorted/", "extra/unsorted-other.txt"));
    }

    #[test]
    fn load_skips_comments_and_blanks() {
        let d = std::env::temp_dir().join(format!("redstring_ignore_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        
        let ig = RsIgnore::load(&d).unwrap();
        assert!(!ig.matches("extra/raw.mp4").unwrap());

        std::fs::write(d.join(IGNORE_FILE), "# big stuff\n\nextra/raw-*.mp4\nextra/unsorted/\n").unwrap();
        let ig = RsIgnore::load(&d).unwrap();
        assert!(ig.matches("extra/raw-interview.mp4").unwrap());
        assert!(ig.matches("extra/unsorted/scan-001.jpg").unwrap());
        assert!(!ig.matches("evidence/docs/contract.pdf").unwrap());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
