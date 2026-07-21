use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::model::EntityKind;
use crate::prefs::config_base;

use super::spec::ModuleSpec;


const BUILTIN: [(&str, &str); 6] = [
    ("builtin:whois", include_str!("../../modules/whois.ron")),
    ("builtin:candel", include_str!("../../modules/candel.ron")),
    ("builtin:dnsx", include_str!("../../modules/dnsx.ron")),
    ("builtin:tlsx", include_str!("../../modules/tlsx.ron")),
    ("builtin:katana", include_str!("../../modules/katana.ron")),
    ("builtin:crtsh", include_str!("../../modules/crtsh.ron")),
];


#[derive(Clone, Debug)]
pub enum ModuleStatus {

    Active(PathBuf),

    Missing(Vec<String>),

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
    let bin_path = which(&spec.bin);
    let mut missing = Vec::new();
    if bin_path.is_none() {
        missing.push(spec.bin.clone());
    }
    for req in &spec.requires {
        if which(req).is_none() {
            missing.push(req.clone());
        }
    }
    let Some(path) = bin_path else {
        return ModuleStatus::Missing(missing);
    };
    if !missing.is_empty() {
        return ModuleStatus::Missing(missing);
    }
    ModuleStatus::Active(path)
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
    use crate::modules::extract::{extractor, sniff};

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
    fn tech_specs_validate_and_resolve_extractors() {
        for origin in ["builtin:dnsx", "builtin:tlsx", "builtin:katana", "builtin:crtsh"] {
            let Some((_, text)) = BUILTIN.iter().find(|(o, _)| *o == origin) else {
                panic!("BUILTIN missing {origin}");
            };
            let entry = entry_from_str(origin, text).unwrap();
            let problems = entry.spec.validate().unwrap();
            assert!(problems.is_empty(), "{origin}: {problems:?}");
            assert!(!entry.spec.shapes.is_empty(), "{origin}: no shapes");
            for shape in &entry.spec.shapes {
                assert!(
                    extractor(&shape.extract.id, &shape.extract.config).is_ok(),
                    "{origin} shape '{}': extractor '{}' does not resolve",
                    shape.id,
                    shape.extract.id
                );
            }
        }
    }

    #[test]
    fn dnsx_ndjson_fans_records_into_infra_pivots() {
        let entry = entry_from_str("builtin:dnsx", BUILTIN[2].1).unwrap();
        let text = "{\"host\":\"example.com\",\"a\":[\"93.184.216.34\"],\"aaaa\":[\"2606:2800:220:1:248:1893:25c8:1946\"],\"ns\":[\"a.iana-servers.net\",\"b.iana-servers.net\"],\"cname\":[\"example.map.cdn.example.\"]}\n";
        let (shape, records) = sniff(&entry.spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::IpAddress,
            label: "93.184.216.34".into(),
            value: "93.184.216.34".into(),
            rel: RelationKind::ResolvesTo,
            outbound: true,
            attrs: Vec::new(),
        }));
        let servers = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Server, rel: RelationKind::ResolvesTo, .. }))
            .count();
        assert_eq!(servers, 2, "both nameservers fan out to server nodes");
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Domain,
            label: "example.map.cdn.example.".into(),
            value: "example.map.cdn.example.".into(),
            rel: RelationKind::PointsTo,
            outbound: true,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn tlsx_ndjson_emits_cert_and_covers_sans() {
        let entry = entry_from_str("builtin:tlsx", BUILTIN[3].1).unwrap();
        let text = "{\"host\":\"example.com\",\"fingerprint\":\"deadbeef\",\"issuer_cn\":\"R3\",\"subject_cn\":\"example.com\",\"subject_an\":[\"example.com\",\"www.example.com\"]}\n";
        let (shape, records) = sniff(&entry.spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::TlsCertificate,
            label: "deadbeef".into(),
            value: "deadbeef".into(),
            rel: RelationKind::PresentsCert,
            outbound: true,
            attrs: vec![
                ("sha256".into(), "deadbeef".into()),
                ("issuer".into(), "R3".into()),
                ("subject".into(), "example.com".into()),
            ],
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Domain,
            label: "www.example.com".into(),
            value: "www.example.com".into(),
            rel: RelationKind::Covers,
            outbound: true,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn katana_ndjson_links_to_crawled_pages() {
        let entry = entry_from_str("builtin:katana", BUILTIN[4].1).unwrap();
        let text = "{\"url\":\"https://example.com/login\"}\n{\"url\":\"https://example.com/about\"}\n";
        let (shape, records) = sniff(&entry.spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        let pages = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::WebPage, rel: RelationKind::LinksTo, .. }))
            .count();
        assert_eq!(pages, 2, "one web page per crawled url");
    }

    #[test]
    fn crtsh_ndjson_covers_each_san() {
        let entry = entry_from_str("builtin:crtsh", BUILTIN[5].1).unwrap();
        let text = "{\"issuer\":\"C=US, O=Let's Encrypt\",\"san\":\"api.example.com\"}\n{\"issuer\":\"C=US, O=Let's Encrypt\",\"san\":\"mail.example.com\"}\n";
        let (shape, records) = sniff(&entry.spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Domain,
            label: "api.example.com".into(),
            value: "api.example.com".into(),
            rel: RelationKind::Covers,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "ct_issuer".into(),
            value: "C=US, O=Let's Encrypt".into(),
        }));
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
        let (shape, records) = sniff(&spec.shapes, ICANN).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr {
            key: "registrar".into(),
            value: "MarkMonitor Inc.".into()
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Organization,
            label: "MarkMonitor Inc.".into(),
            value: "MarkMonitor Inc.".into(),
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
        let (shape, records) = sniff(&spec.shapes, APNIC).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Organization,
            label: "APNIC Research and Development".into(),
            value: "APNIC Research and Development".into(),
            rel: RelationKind::RegisteredAt,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Email,
            label: "helpdesk@apnic.net".into(),
            value: "helpdesk@apnic.net".into(),
            rel: RelationKind::RelatedTo,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Location,
            label: "AU".into(),
            value: "AU".into(),
            rel: RelationKind::LocatedAt,
            outbound: true,
            attrs: vec![("country_code".into(), "AU".into())],
        }));
    }

    #[test]
    fn arin_shape_coalesces_geo_into_location() {
        let spec = whois_spec();
        let arin_with_city = format!("{ARIN}City:           Mountain View\nStateProv:      CA\n");
        let (shape, records) = sniff(&spec.shapes, &arin_with_city).unwrap().unwrap();
        assert_eq!(shape.id, "arin");
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Location,
            label: "US".into(),
            value: "US".into(),
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

    fn probe_spec(bin: &str, requires: &[&str]) -> ModuleSpec {
        ModuleSpec {
            id: "t".into(),
            label: "t".into(),
            applies_to: vec!["domain".into()],
            bin: bin.into(),
            requires: requires.iter().map(|s| (*s).to_string()).collect(),
            args: Vec::new(),
            timeout_secs: 5,
            keys: Vec::new(),
            shapes: Vec::new(),
        }
    }

    #[test]
    fn requires_gate_names_missing_pipeline_bin() {
        let spec = probe_spec("sh", &["definitely_not_a_real_binary_xyz"]);
        let ModuleStatus::Missing(missing) = module_status(&spec, Vec::new()) else {
            panic!("a required pipeline bin being absent must yield Missing");
        };
        assert!(missing.iter().any(|m| m == "definitely_not_a_real_binary_xyz"));
        assert!(!missing.iter().any(|m| m == "sh"));
    }

    #[test]
    fn bin_and_requires_all_present_is_active() {
        let spec = probe_spec("sh", &["sh"]);
        let ModuleStatus::Active(_) = module_status(&spec, Vec::new()) else {
            panic!("sh present with requires=[sh] must be Active");
        };
    }

    #[test]
    fn missing_bin_is_named_in_missing_list() {
        let spec = probe_spec("definitely_not_a_real_binary_xyz", &[]);
        let ModuleStatus::Missing(missing) = module_status(&spec, Vec::new()) else {
            panic!("an absent bin must yield Missing");
        };
        assert!(missing.iter().any(|m| m == "definitely_not_a_real_binary_xyz"));
    }

    #[test]
    fn katana_now_applies_to_domain() {
        let entry = entry_from_str("builtin:katana", BUILTIN[4].1).unwrap();
        assert!(entry.spec.kinds().unwrap().contains(&EntityKind::Domain));
    }
}
