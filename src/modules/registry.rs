use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::model::EntityKind;
use crate::prefs::config_base;

use super::spec::ModuleSpec;


const BUILTIN: [(&str, &str); 1] = [("builtin:whois", include_str!("../../modules/whois.ron"))];


#[derive(Clone, Debug)]
pub enum ModuleStatus {

    Active(PathBuf),

    Missing,

    Broken(String),
}


pub struct ModuleEntry {
    pub spec: ModuleSpec,
    pub origin: String,
    pub status: ModuleStatus,
}


pub struct Registry {
    pub entries: Vec<ModuleEntry>,
}

impl Registry {

    pub fn load() -> Result<Self, Error> {
        let mut entries = Vec::new();
        for (origin, text) in BUILTIN {
            entries.push(entry_from_str(origin, text)?);
        }
        for (origin, text) in user_specs()? {
            entries.push(entry_from_str(&origin, &text)?);
        }
        Ok(Self { entries })
    }


    pub fn for_kind(&self, kind: EntityKind) -> Result<Vec<usize>, Error> {
        let mut out = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            let ModuleStatus::Active(_) = &e.status else {
                continue;
            };
            if e.spec.kinds()?.contains(&kind) {
                out.push(i);
            }
        }
        Ok(out)
    }
}

fn entry_from_str(origin: &str, text: &str) -> Result<ModuleEntry, Error> {
    let spec: ModuleSpec = ron::from_str(text).map_err(|source| Error::RonParse {
        path: PathBuf::from(origin),
        source,
    })?;
    let problems = spec.validate()?;
    let status = module_status(&spec, problems);
    Ok(ModuleEntry {
        spec,
        origin: origin.to_owned(),
        status,
    })
}

fn module_status(spec: &ModuleSpec, problems: Vec<String>) -> ModuleStatus {
    if !problems.is_empty() {
        return ModuleStatus::Broken(problems.join("; "));
    }
    let Some(p) = which(&spec.bin) else {
        return ModuleStatus::Missing;
    };
    ModuleStatus::Active(p)
}

fn user_specs() -> Result<Vec<(String, String)>, Error> {
    let dir = config_base()?.join("redstring").join("modules");
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(Error::Io { path: dir, source }),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "ron") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        out.push((path.display().to_string(), text));
    }
    Ok(out)
}

fn which(bin: &str) -> Option<PathBuf> {
    if bin.contains('/') {
        let p = PathBuf::from(bin);
        if is_exec(&p) {
            return Some(p);
        }
        return None;
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(bin);
        if is_exec(&cand) {
            return Some(cand);
        }
    }
    None
}

fn is_exec(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let Ok(md) = std::fs::metadata(p) else {
        return false;
    };
    md.is_file() && md.permissions().mode() & 0o111 != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RelationKind;
    use crate::modules::emit::{Fact, emit};
    use crate::modules::extract::sniff;

    const APNIC: &str = "\
% [whois.apnic.net]
% Information related to '1.1.1.0 - 1.1.1.255'
inetnum:        1.1.1.0 - 1.1.1.255
netname:        APNIC-LABS
descr:          APNIC and Cloudflare DNS Resolver project
country:        AU
abuse-mailbox:  helpdesk@apnic.net
org-name:       APNIC Research and Development
source:         APNIC
";

    const ICANN: &str = "\
   Domain Name: GOOGLE.COM
   Registrar: MarkMonitor Inc.
   Creation Date: 1997-09-15T04:00:00Z
   Registry Expiry Date: 2028-09-14T04:00:00Z
   Name Server: NS1.GOOGLE.COM
   Name Server: NS2.GOOGLE.COM
   DNSSEC: unsigned
";

    const ARIN: &str = "\
# start
NetRange:       8.8.8.0 - 8.8.8.255
CIDR:           8.8.8.0/24
NetName:        GOGL
Country:        US
OrgName:        Google LLC
";

    fn whois_spec() -> ModuleSpec {
        let entry = entry_from_str("builtin:whois", BUILTIN[0].1).unwrap();
        entry.spec
    }

    #[test]
    fn builtin_specs_parse_and_validate_clean() {
        for (origin, text) in BUILTIN {
            let entry = entry_from_str(origin, text).unwrap();
            let problems = entry.spec.validate().unwrap();
            assert!(problems.is_empty(), "{origin}: {problems:?}");
        }
    }

    #[test]
    fn sniff_picks_the_right_registry_shape() {
        let spec = whois_spec();
        let (shape, _) = sniff(&spec.shapes, ICANN).unwrap().unwrap();
        assert_eq!(shape.id, "icann");
        let (shape, _) = sniff(&spec.shapes, APNIC).unwrap().unwrap();
        assert_eq!(shape.id, "rpsl");
        let (shape, _) = sniff(&spec.shapes, ARIN).unwrap().unwrap();
        assert_eq!(shape.id, "arin");
        assert!(sniff(&spec.shapes, "garbage output\nnothing here\n").unwrap().is_none());
    }

    #[test]
    fn icann_shape_emits_registrar_and_nameservers() {
        let spec = whois_spec();
        let (shape, vars) = sniff(&spec.shapes, ICANN).unwrap().unwrap();
        let facts = emit(shape, &vars).unwrap();
        assert!(facts.contains(&Fact::Attr {
            key: "registrar".into(),
            value: "MarkMonitor Inc.".into()
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Company,
            label: "MarkMonitor Inc.".into(),
            rel: RelationKind::RegisteredAt,
            outbound: true,
            attrs: Vec::new(),
        }));
        let servers = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Server, .. }))
            .count();
        assert_eq!(servers, 2);
    }

    #[test]
    fn rpsl_shape_emits_org_abuse_contact_and_location() {
        let spec = whois_spec();
        let (shape, vars) = sniff(&spec.shapes, APNIC).unwrap().unwrap();
        let facts = emit(shape, &vars).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Organization,
            label: "APNIC Research and Development".into(),
            rel: RelationKind::RegisteredAt,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Email,
            label: "helpdesk@apnic.net".into(),
            rel: RelationKind::RelatedTo,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Location,
            label: "AU".into(),
            rel: RelationKind::LocatedAt,
            outbound: true,
            attrs: vec![("country_code".into(), "AU".into())],
        }));
    }

    #[test]
    fn arin_shape_coalesces_geo_into_location() {
        let spec = whois_spec();
        let arin_with_city = format!("{ARIN}City:           Mountain View\nStateProv:      CA\n");
        let (shape, vars) = sniff(&spec.shapes, &arin_with_city).unwrap().unwrap();
        assert_eq!(shape.id, "arin");
        let facts = emit(shape, &vars).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Location,
            label: "US".into(),
            rel: RelationKind::LocatedAt,
            outbound: true,
            attrs: vec![
                ("country_code".into(), "US".into()),
                ("city".into(), "Mountain View".into()),
                ("region".into(), "CA".into()),
            ],
        }));
        assert!(facts.contains(&Fact::Attr { key: "cidr".into(), value: "8.8.8.0/24".into() }));
    }

    #[test]
    fn which_finds_sh_and_rejects_nonsense() {
        assert!(which("sh").is_some());
        assert!(which("definitely_not_a_real_binary_xyz").is_none());
    }
}
