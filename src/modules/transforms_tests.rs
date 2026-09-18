#[cfg(test)]
mod transforms_tests {
    use super::*;
    use crate::model::{ObsValue, RelationKind};
    use crate::modules::emit::{Fact, emit, parse_obs_value};
    use crate::modules::extract::sniff;
    use crate::modules::run::streaming;
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
            args[1].contains("shodan host \"$1\""),
            "the script references the value positionally, never inlines it"
        );
        assert!(
            !args[1].contains("8.8.8.8"),
            "the value never lands in the script text"
        );
        assert_eq!(
            args.last().unwrap(),
            "8.8.8.8",
            "the value is bound as a real argument the shell cannot reparse"
        );
    }

    #[test]
    fn hostile_value_is_bound_not_executed() {
        let spec = builtin("builtin:shodan");
        let hostile = "x\"; id; echo \"";
        let args = spec.render_args(hostile, "evil", "evil", &[]).unwrap();
        assert!(
            !args[1].contains(hostile),
            "the hostile text never reaches the script"
        );
        assert!(
            !args[1].contains("; id;"),
            "no injected command lands in the script"
        );
        assert_eq!(
            args.last().unwrap(),
            hostile,
            "the hostile text survives intact as inert data"
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
            args[1].trim_start().starts_with("binwalk \"$1\""),
            "the file observation is bound positionally, with no flag before it"
        );
        assert_eq!(args.last().unwrap(), "evidence/docs/firmware.bin");
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
    fn alibi_applies_to_web_targets_and_stays_read_only() {
        let spec = builtin("builtin:alibi");
        let kinds = spec.kinds().unwrap();
        assert!(kinds.contains(&EntityKind::WebPage));
        assert!(kinds.contains(&EntityKind::Domain));
        assert_eq!(spec.bin, "alibi", "alibi stays an external executable");
        assert!(
            spec.args.contains(&"--check".to_owned()),
            "the module must never submit — --check is read-only"
        );
        assert!(
            spec.args.contains(&"--json".to_owned()),
            "JSONL is the parsed channel"
        );
    }

    #[test]
    fn alibi_confirmed_row_emits_one_archived_page_labelled_by_host() {
        let spec = builtin("builtin:alibi");
        let text = "{\"witness\":\"wayback\",\"url\":\"https://example.com/\",\"status\":\"confirmed\",\"reason\":\"ok\",\"snapshot\":\"https://web.archive.org/web/20260101000000/https://example.com/\",\"ts\":\"20260101000000\",\"detail\":null}\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        assert_eq!(shape.id, "alibi");
        let facts = emit(shape, &records).unwrap();
        assert_eq!(facts.len(), 1, "exactly one node for one confirmed witness");
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::WebPage,
            label: "web.archive.org".into(),
            value: "https://web.archive.org/web/20260101000000/https://example.com/".into(),
            rel: RelationKind::ArchivedAs,
            outbound: true,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn alibi_null_snapshot_rows_emit_nothing() {
        let spec = builtin("builtin:alibi");
        let text = "{\"witness\":\"wayback\",\"url\":\"https://example.com/\",\"status\":\"error\",\"reason\":\"network\",\"snapshot\":null,\"ts\":null,\"detail\":null}\n{\"witness\":\"archivetoday\",\"url\":\"https://example.com/\",\"status\":\"error\",\"reason\":\"network\",\"snapshot\":null,\"ts\":null,\"detail\":null}\n{\"witness\":\"ghostarchive\",\"url\":\"https://example.com/\",\"status\":\"error\",\"reason\":\"network\",\"snapshot\":null,\"ts\":null,\"detail\":null}\n";
        let parsed = sniff(&spec.shapes, text).unwrap();
        let facts = match parsed {
            Some((shape, records)) => emit(shape, &records).unwrap(),
            None => Vec::new(),
        };
        assert!(
            facts.is_empty(),
            "the offline exit-2 shape yields zero entities and does not error"
        );
    }

    #[test]
    fn alibi_mixed_batch_emits_only_the_confirmed_witness() {
        let spec = builtin("builtin:alibi");
        let text = "{\"witness\":\"wayback\",\"url\":\"https://example.com/\",\"status\":\"confirmed\",\"reason\":\"ok\",\"snapshot\":\"https://web.archive.org/web/20260101000000/https://example.com/\",\"ts\":\"20260101000000\",\"detail\":null}\n{\"witness\":\"archivetoday\",\"url\":\"https://example.com/\",\"status\":\"none\",\"reason\":\"absent\",\"snapshot\":null,\"ts\":null,\"detail\":null}\n{\"witness\":\"ghostarchive\",\"url\":\"https://example.com/\",\"status\":\"error\",\"reason\":\"server_error\",\"snapshot\":null,\"ts\":null,\"detail\":\"http 503\"}\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert_eq!(facts.len(), 1, "the exit-1 shape still puts the one hit on the board");
    }

    #[test]
    fn alibi_streams_so_partial_results_survive_a_nonzero_exit() {
        let spec = builtin("builtin:alibi");
        assert!(
            streaming(&spec.shapes).unwrap(),
            "ndjson makes alibi stream: facts are applied per line as they land, \
             so exit 1 and 2 keep their confirmed rows instead of being dropped"
        );
        assert!(
            !streaming(&builtin("builtin:wayback").shapes).unwrap(),
            "the lines-based wayback module is batch, parsed only on exit 0"
        );
    }

    #[test]
    fn alibi_and_wayback_stay_distinct_moves() {
        let alibi = builtin("builtin:alibi");
        let wayback = builtin("builtin:wayback");
        assert_ne!(alibi.bin, wayback.bin, "alibi is its own executable");
        assert!(
            wayback.args.iter().any(|a| a.contains("cdx")),
            "wayback sweeps the CDX index for historical URLs — a discovery move"
        );
        assert!(
            alibi.args.contains(&"--check".to_owned()),
            "alibi verifies one URL across three archives — a verification move"
        );
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
            "the first announced cidr lands as an observation"
        );
        assert!(
            facts.contains(&Fact::Attr {
                key: "netblock".into(),
                value: "8.8.8.0/24".into(),
                kind: ObsKind::Text,
            }),
            "every announced cidr now lands — the range array is no longer truncated to the first"
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

    #[test]
    fn dehashed_applies_to_identity_kinds_and_declares_its_api_key_header() {
        let spec = builtin("builtin:dehashed");
        assert!(spec.keys.iter().any(|k| k == "dehashed"), "dehashed grows its own key slot");
        assert_eq!(
            spec.headers,
            vec![("DeHashed-Api-Key".to_owned(), "{key:dehashed}".to_owned())],
            "the secret rides the headers field as an env token, never argv"
        );
        let kinds = spec.kinds().unwrap();
        for want in [
            EntityKind::Email,
            EntityKind::Person,
            EntityKind::Phone,
            EntityKind::Alias,
            EntityKind::SocialAccount,
        ] {
            assert!(kinds.contains(&want), "dehashed should apply to {want:?}");
        }
    }

    #[test]
    fn dehashed_breach_rows_pivot_to_typed_nodes_and_keep_hashes_descriptive() {
        let spec = builtin("builtin:dehashed");
        let sample = r#"{"balance":42,"entries":[
            {"id":"a1","email":["john.doe@example.com"],"username":["johnd"],
             "password":["hunter2"],"hashed_password":["$2y$10$Abc"],"name":["John Doe"],
             "phone":["+15551234567"],"address":["221B Baker St"],
             "database_name":"ExampleBreach2019","ip_address":["203.0.113.7"]},
            {"id":"b2","email":["john.doe@example.com"],"password":["p@ssw0rd"],
             "database_name":"AnotherLeak2021"}
        ],"total":2,"success":true}"#;
        let (shape, records) = sniff(&spec.shapes, sample).unwrap().unwrap();
        assert_eq!(shape.id, "dehashed");
        assert_eq!(records.len(), 2, "one record per breach row");
        let facts = emit(shape, &records).unwrap();

        let leak = |name: &str| Fact::Entity {
            kind: EntityKind::Leak,
            label: name.to_owned(),
            value: name.to_owned(),
            rel: RelationKind::AppearedIn,
            outbound: true,
            attrs: Vec::new(),
        };
        assert!(facts.contains(&leak("ExampleBreach2019")));
        assert!(facts.contains(&leak("AnotherLeak2021")), "each distinct breach is its own Leak node");

        let related = |kind: EntityKind, v: &str| Fact::Entity {
            kind,
            label: v.to_owned(),
            value: v.to_owned(),
            rel: RelationKind::RelatedTo,
            outbound: true,
            attrs: Vec::new(),
        };
        assert!(facts.contains(&related(EntityKind::Email, "john.doe@example.com")));
        assert!(facts.contains(&related(EntityKind::Alias, "johnd")));
        assert!(facts.contains(&related(EntityKind::Person, "John Doe")));
        assert!(facts.contains(&related(EntityKind::Phone, "+15551234567")));
        assert!(facts.contains(&related(EntityKind::Address, "221B Baker St")));
        assert!(facts.contains(&related(EntityKind::IpAddress, "203.0.113.7")));
        assert!(facts.contains(&related(EntityKind::Secret, "hunter2")), "plaintext password reuse pivots");
        assert!(
            facts.contains(&related(EntityKind::Secret, "p@ssw0rd")),
            "the second row's distinct password is its own Secret"
        );

        assert!(
            facts.contains(&Fact::Attr {
                key: "hashed_password".into(),
                value: "$2y$10$Abc".into(),
                kind: ObsKind::Text,
            }),
            "a salted hash is a descriptive observation, not a pivot"
        );
        assert!(
            !facts.iter().any(|f| matches!(
                f,
                Fact::Entity { kind: EntityKind::Secret, value, .. } if value.starts_with("$2y$")
            )),
            "the hash must never become a Secret node"
        );
    }

    #[test]
    fn opensanctions_applies_to_person_and_org_with_api_key_header() {
        let spec = builtin("builtin:opensanctions");
        assert!(spec.keys.iter().any(|k| k == "opensanctions"), "opensanctions grows its own key slot");
        assert_eq!(
            spec.headers,
            vec![("Authorization".to_owned(), "ApiKey {key:opensanctions}".to_owned())],
            "the api key rides the headers field as an env token, never argv"
        );
        let kinds = spec.kinds().unwrap();
        assert!(kinds.contains(&EntityKind::Person), "screens people");
        assert!(kinds.contains(&EntityKind::Organization), "screens organizations");
    }

    #[test]
    fn opensanctions_renders_urlencoded_search_query() {
        let spec = builtin("builtin:opensanctions");
        let args = spec
            .render_args("Vladimir Putin", "Vladimir Putin", "vladimir_putin", &[])
            .unwrap();
        assert!(
            args.contains(&"q=Vladimir Putin".to_owned()),
            "the name rides through --data-urlencode for curl to percent-encode: {args:?}"
        );
        assert!(args.contains(&"https://api.opensanctions.org/search/default".to_owned()));
    }

    #[test]
    fn opensanctions_key_slot_joins_the_settings_union() {
        let mut entries = Vec::new();
        for (origin, text) in BUILTIN {
            entries.push(entry_from_str(origin, text).unwrap());
        }
        let ids = Registry { entries }.key_ids().unwrap();
        assert!(ids.iter().any(|k| k == "opensanctions"), "the keyed slot surfaces in Settings: {ids:?}");
        assert!(ids.iter().any(|k| k == "dehashed"), "the union keeps every other keyed module too");
    }

    #[test]
    fn opensanctions_hits_pivot_watchlists_and_annotate_the_subject() {
        let spec = builtin("builtin:opensanctions");
        let sample = r#"{"total":{"value":2},"results":[
            {"id":"Q7747","caption":"Vladimir Vladimirovich Putin","schema":"Person",
             "properties":{"topics":["sanction","role.pep"],"country":["ru"],"birthDate":["1952-10-07"]},
             "datasets":["eu_fsf","us_ofac_sdn","gb_hmt_sanctions"],"score":0.95},
            {"id":"NK-abc","caption":"Vostok Trading LLC","schema":"Organization",
             "properties":{"topics":["sanction"],"country":["ru"]},
             "datasets":["eu_fsf"],"score":0.6}
        ]}"#;
        let (shape, records) = sniff(&spec.shapes, sample).unwrap().unwrap();
        assert_eq!(shape.id, "opensanctions");
        assert_eq!(records.len(), 2, "one record per matched watchlisted entity");
        let facts = emit(shape, &records).unwrap();

        assert!(facts.contains(&Fact::Attr {
            key: "sanctions_match".into(),
            value: "Vladimir Vladimirovich Putin".into(),
            kind: ObsKind::Text,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "match_schema".into(),
            value: "Person".into(),
            kind: ObsKind::Text,
        }));
        assert!(
            facts.contains(&Fact::Attr { key: "match_score".into(), value: "0.95".into(), kind: ObsKind::Number }),
            "the match confidence is a typed Number, not a string"
        );
        assert!(
            facts.contains(&Fact::Attr { key: "topics".into(), value: "sanction".into(), kind: ObsKind::Text }),
            "the sanction risk tag surfaces as an observation"
        );
        assert!(
            facts.contains(&Fact::Attr { key: "topics".into(), value: "role.pep".into(), kind: ObsKind::Text }),
            "every risk tag now surfaces — the pep tag is no longer trapped in the archived raw"
        );
        assert!(facts.contains(&Fact::Attr {
            key: "country".into(),
            value: "ru".into(),
            kind: ObsKind::Text,
        }));

        let watchlist = |slug: &str| Fact::Entity {
            kind: EntityKind::Dataset,
            label: slug.to_owned(),
            value: slug.to_owned(),
            rel: RelationKind::AppearedIn,
            outbound: true,
            attrs: Vec::new(),
        };
        assert!(facts.contains(&watchlist("us_ofac_sdn")), "each source dataset is a watchlist Dataset node");
        assert!(facts.contains(&watchlist("eu_fsf")));
        assert!(facts.contains(&watchlist("gb_hmt_sanctions")));
        let watchlists = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Dataset, rel: RelationKind::AppearedIn, .. }))
            .count();
        assert_eq!(watchlists, 4, "datasets fan out multi-value: 3 for the person plus 1 for the org");

        assert!(facts.contains(&Fact::Attr {
            key: "sanctions_match".into(),
            value: "Vostok Trading LLC".into(),
            kind: ObsKind::Text,
        }));
    }
}
