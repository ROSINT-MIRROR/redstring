#[cfg(test)]
mod transforms_tests {
    use super::*;
    use crate::model::{ObsValue, RelationKind};
    use crate::modules::emit::{Fact, emit, parse_obs_value};
    use crate::modules::extract::sniff;
    use crate::modules::ObsKind;

    fn builtin(origin: &str) -> ModuleSpec {
        let Some((_, text)) = BUILTIN.iter().find(|(o, _)| *o == origin) else {
            panic!("BUILTIN missing {origin}");
        };
        entry_from_str(origin, text).unwrap().spec
    }

    #[test]
    fn shodan_applies_to_ip_only() {
        let kinds = builtin("builtin:shodan").kinds().unwrap();
        assert!(kinds.contains(&EntityKind::IpAddress));
        assert_eq!(kinds.len(), 1);
    }

    #[test]
    fn shodan_reshaped_host_pivots_domains_org_and_ports() {
        let spec = builtin("builtin:shodan");
        let text = "hostname: presence-ny1.liveops.com\nhostname: dns.google\ncity: Mountain View\ncountry: United States\norganization: Google LLC\nopen_ports: 53, 443\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        assert_eq!(shape.id, "shodan");
        let facts = emit(shape, &records).unwrap();
        assert!(
            facts.contains(&Fact::Entity {
                kind: EntityKind::Domain,
                label: "dns.google".into(),
                value: "dns.google".into(),
                rel: RelationKind::ResolvesTo,
                outbound: false,
                attrs: Vec::new(),
            }),
            "a hostname resolves TO the address — inbound edge"
        );
        let domains = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Domain, .. }))
            .count();
        assert_eq!(domains, 2, "both hostnames fan out to domain nodes");
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Organization,
            label: "Google LLC".into(),
            value: "Google LLC".into(),
            rel: RelationKind::RelatedTo,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "open_ports".into(),
            value: "53, 443".into(),
            kind: ObsKind::Text,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "country".into(),
            value: "United States".into(),
            kind: ObsKind::Text,
        }));
    }

    #[test]
    fn shodan_reshaped_host_lifts_services_vulns_and_os() {
        let spec = builtin("builtin:shodan");
        let text = "hostname: web1.evil.example\norganization: Evil Hosting GmbH\nos: Linux 3.x\nvulns: CVE-2021-41773, CVE-2014-0160\nopen_ports: 22, 80, 443\nservices: OpenSSH, nginx\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr {
            key: "services".into(),
            value: "OpenSSH, nginx".into(),
            kind: ObsKind::Text,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "vulns".into(),
            value: "CVE-2021-41773, CVE-2014-0160".into(),
            kind: ObsKind::Text,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "os".into(),
            value: "Linux 3.x".into(),
            kind: ObsKind::Text,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "open_ports".into(),
            value: "22, 80, 443".into(),
            kind: ObsKind::Text,
        }));
    }

    #[test]
    fn shodan_renders_host_over_validated_value() {
        let spec = builtin("builtin:shodan");
        let args = spec.render_args("8.8.8.8", "dns.google", "8_8_8_8", &[]).unwrap();
        assert_eq!(args[0], "-c");
        assert!(
            args[1].contains("shodan host \"8.8.8.8\""),
            "the validated IP value is substituted into the host lookup"
        );
    }

    #[test]
    fn binwalk_applies_to_document_and_dataset() {
        let kinds = builtin("builtin:binwalk").kinds().unwrap();
        assert!(kinds.contains(&EntityKind::Document));
        assert!(kinds.contains(&EntityKind::Dataset));
    }

    #[test]
    fn binwalk_reshaped_scan_flags_signatures_and_notable_finds() {
        let spec = builtin("builtin:binwalk");
        let text = "embedded_signatures: 5\nsignature_types: gzip compressed data, Squashfs filesystem, PEM certificate, PEM private key, ZIP archive\ncompressed_data: yes\nfilesystems: yes\ncertificates: yes\nprivate_keys: yes\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        assert_eq!(shape.id, "binwalk");
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr {
            key: "embedded_signatures".into(),
            value: "5".into(),
            kind: ObsKind::Number,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "signature_types".into(),
            value: "gzip compressed data, Squashfs filesystem, PEM certificate, PEM private key, ZIP archive".into(),
            kind: ObsKind::Text,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "has_compressed_data".into(),
            value: "yes".into(),
            kind: ObsKind::Bool,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "has_filesystem".into(),
            value: "yes".into(),
            kind: ObsKind::Bool,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "has_certificate".into(),
            value: "yes".into(),
            kind: ObsKind::Bool,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "has_private_key".into(),
            value: "yes".into(),
            kind: ObsKind::Bool,
        }));
    }

    #[test]
    fn binwalk_clean_file_sets_only_present_flags() {
        let spec = builtin("builtin:binwalk");
        let text = "embedded_signatures: 1\nsignature_types: ELF binary\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr {
            key: "embedded_signatures".into(),
            value: "1".into(),
            kind: ObsKind::Number,
        }));
        assert!(
            !facts
                .iter()
                .any(|f| matches!(f, Fact::Attr { key, .. } if key == "has_compressed_data")),
            "a flag is emitted only when the reshape line is present"
        );
    }

    #[test]
    fn binwalk_renders_signature_scan_over_project_relative_file() {
        let spec = builtin("builtin:binwalk");
        let args = spec
            .render_args("", "firmware.bin", "firmware_bin", &[("file".into(), "evidence/docs/firmware.bin".into())])
            .unwrap();
        assert_eq!(args[0], "-c");
        assert!(
            args[1].trim_start().starts_with("binwalk \"evidence/docs/firmware.bin\""),
            "the file observation is substituted, project-relative, with no flag before it"
        );
        assert!(!args[1].contains("--extract"), "signature scan only — never carves files");
    }

    #[test]
    fn subfinder_lines_fan_out_to_related_domains() {
        let spec = builtin("builtin:subfinder");
        let text = "api.example.com\nmail.example.com\nvpn.example.com\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        assert_eq!(shape.id, "subfinder");
        let facts = emit(shape, &records).unwrap();
        let domains = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Domain, .. }))
            .count();
        assert_eq!(domains, 3, "one domain node per discovered subdomain line");
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Domain,
            label: "api.example.com".into(),
            value: "api.example.com".into(),
            rel: RelationKind::RelatedTo,
            outbound: true,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn gau_lines_become_related_web_pages() {
        let spec = builtin("builtin:gau");
        let text = "https://example.com/\nhttps://example.com/login?next=/admin\nhttps://cdn.example.com/app.js\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        assert_eq!(shape.id, "gau");
        let facts = emit(shape, &records).unwrap();
        let pages = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::WebPage, .. }))
            .count();
        assert_eq!(pages, 3, "one web page per historical URL");
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::WebPage,
            label: "https://example.com/login?next=/admin".into(),
            value: "https://example.com/login?next=/admin".into(),
            rel: RelationKind::RelatedTo,
            outbound: true,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn candel_ndjson_hits_use_website_accounts_labeled_by_host() {
        let spec = builtin("builtin:candel");
        let text = "{\"slug\":\"github\",\"url\":\"https://github.com/torvalds\"}\n{\"slug\":\"reddit\",\"url\":\"https://www.reddit.com/user/torvalds\"}\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        assert_eq!(shape.id, "candel");
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Website,
            label: "github.com".into(),
            value: "https://github.com/torvalds".into(),
            rel: RelationKind::UsesAccount,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(
            facts.contains(&Fact::Entity {
                kind: EntityKind::Website,
                label: "reddit.com".into(),
                value: "https://www.reddit.com/user/torvalds".into(),
                rel: RelationKind::UsesAccount,
                outbound: true,
                attrs: Vec::new(),
            }),
            "the account host drops the www prefix while the full url stays the value"
        );
    }

    #[test]
    fn asnmap_ndjson_lifts_org_country_and_first_netblock() {
        let spec = builtin("builtin:asnmap");
        let text = "{\"timestamp\":\"2023-03-23T21:22:56Z\",\"input\":\"AS15169\",\"as_number\":\"AS15169\",\"as_name\":\"GOOGLE\",\"as_country\":\"US\",\"as_range\":[\"8.8.4.0/24\",\"8.8.8.0/24\"]}\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        assert_eq!(shape.id, "asnmap");
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr {
            key: "as_org".into(),
            value: "GOOGLE".into(),
            kind: ObsKind::Text,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "country".into(),
            value: "US".into(),
            kind: ObsKind::Text,
        }));
        assert!(
            facts.contains(&Fact::Attr {
                key: "netblock".into(),
                value: "8.8.4.0/24".into(),
                kind: ObsKind::Text,
            }),
            "the announced range array lands as an observation, first cidr wins"
        );
    }

    #[test]
    fn exiftool_gps_emits_a_typed_coordinate_and_keeps_string_metadata() {
        let spec = builtin("builtin:exiftool");
        let text = "[{\"SourceFile\":\"evidence/docs/geo.jpg\",\"Artist\":\"Jane Doe\",\"CreateDate\":\"2021:06:15 12:30:00\",\"Model\":\"Canon EOS\",\"GPSPosition\":\"40.741895 -73.989308\"}]";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        assert_eq!(shape.id, "exiftool");
        let facts = emit(shape, &records).unwrap();
        assert!(
            facts.contains(&Fact::Attr {
                key: "coordinates".into(),
                value: "40.741895 -73.989308".into(),
                kind: ObsKind::Coord,
            }),
            "GPS rides the source node as a typed Coord, not an untyped location entity"
        );
        assert!(
            facts.contains(&Fact::Entity {
                kind: EntityKind::Location,
                label: "40.741895 -73.989308".into(),
                value: "40.741895 -73.989308".into(),
                rel: RelationKind::LocatedAt,
                outbound: true,
                attrs: Vec::new(),
            }),
            "the same -n coordinate also anchors a located_at pin, now a clean sluggable value"
        );
        assert_eq!(
            parse_obs_value(ObsKind::Coord, "40.741895 -73.989308").unwrap(),
            ObsValue::Coord { lat: 40.741895, lon: -73.989308 },
            "exiftool -n renders a signed decimal pair that parses to a real point"
        );
        assert!(facts.contains(&Fact::Attr {
            key: "author".into(),
            value: "Jane Doe".into(),
            kind: ObsKind::Text,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "device_model".into(),
            value: "Canon EOS".into(),
            kind: ObsKind::Text,
        }));
    }
}
