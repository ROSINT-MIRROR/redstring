#[cfg(test)]
mod key_ids_tests {
    use super::{ModuleEntry, ModuleStatus, Registry};
    use crate::modules::spec::ModuleSpec;

    fn spec_with_keys(id: &str, keys: &[&str]) -> ModuleSpec {
        ModuleSpec {
            id: id.to_owned(),
            label: id.to_owned(),
            applies_to: vec!["email".to_owned()],
            bin: "true".to_owned(),
            requires: Vec::new(),
            args: vec!["{value}".to_owned()],
            timeout_secs: 20,
            keys: keys.iter().map(|k| (*k).to_owned()).collect(),
            headers: Vec::new(),
            shapes: Vec::new(),
        }
    }

    fn entry(spec: ModuleSpec) -> ModuleEntry {
        ModuleEntry {
            spec,
            origin: "<test>".to_owned(),
            status: ModuleStatus::Broken("test".to_owned()),
        }
    }

    #[test]
    fn key_ids_unions_across_modules_dedup_stable() {
        let registry = Registry {
            entries: vec![
                entry(spec_with_keys("dh", &["dehashed"])),
                entry(spec_with_keys("shodan", &["shodan", "dehashed"])),
                entry(spec_with_keys("whois", &[])),
            ],
        };
        let ids = registry.key_ids().unwrap();
        assert_eq!(ids, vec!["dehashed".to_owned(), "shodan".to_owned()]);
    }

    #[test]
    fn key_ids_is_empty_when_no_module_declares_a_key() {
        let registry = Registry {
            entries: vec![entry(spec_with_keys("whois", &[]))],
        };
        assert!(registry.key_ids().unwrap().is_empty());
    }

    #[test]
    fn key_ids_reports_a_single_declared_key() {
        let registry = Registry {
            entries: vec![entry(spec_with_keys("dh", &["dehashed"]))],
        };
        assert_eq!(registry.key_ids().unwrap(), vec!["dehashed".to_owned()]);
    }

    #[test]
    fn key_ids_ignores_module_status_and_still_unions() {
        let missing = ModuleEntry {
            spec: spec_with_keys("dh", &["dehashed"]),
            origin: "<test>".to_owned(),
            status: ModuleStatus::Missing(vec!["dh".to_owned()]),
        };
        let registry = Registry { entries: vec![missing] };
        assert_eq!(registry.key_ids().unwrap(), vec!["dehashed".to_owned()]);
    }
}
