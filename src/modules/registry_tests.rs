#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RelationKind;
    use crate::modules::emit::{Fact, emit};
    use crate::modules::extract::{extractor, sniff};
    use crate::modules::ObsKind;

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

    const ROTLD: &str = "\
% ROTLD legal preamble, no colon on this line
  Domain Name: google.ro
  Registered On: 2000-07-17
  Registrar: MarkMonitor Inc.
  DNSSEC: Inactive
  Nameserver: ns1.google.com
  Nameserver: ns2.google.com
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
                ("sha256".into(), "deadbeef".into(), ObsKind::Text),
                ("issuer".into(), "R3".into(), ObsKind::Text),
                ("subject".into(), "example.com".into(), ObsKind::Text),
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
            value: "C=US, O=Let's Encrypt".into(), kind: ObsKind::Text, }));
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
            value: "MarkMonitor Inc.".into(), kind: ObsKind::Text }));
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
            attrs: vec![("country_code".into(), "AU".into(), ObsKind::Text)],
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
                ("country_code".into(), "US".into(), ObsKind::Text),
                ("city".into(), "Mountain View".into(), ObsKind::Text),
                ("region".into(), "CA".into(), ObsKind::Text),
            ],
        }));
        assert!(facts.contains(&Fact::Attr { key: "cidr".into(), value: "8.8.8.0/24".into(), kind: ObsKind::Text }));
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

    fn builtin(origin: &str) -> ModuleSpec {
        let Some((_, text)) = BUILTIN.iter().find(|(o, _)| *o == origin) else {
            panic!("BUILTIN missing {origin}");
        };
        entry_from_str(origin, text).unwrap().spec
    }

    #[test]
    fn webprobe_and_hrefs_apply_to_web_kinds() {
        let wp = builtin("builtin:webprobe").kinds().unwrap();
        assert!(wp.contains(&EntityKind::Domain));
        assert!(wp.contains(&EntityKind::Website));
        let hr = builtin("builtin:hrefs").kinds().unwrap();
        assert!(hr.contains(&EntityKind::Website));
        assert!(hr.contains(&EntityKind::WebPage));
    }

    #[test]
    fn webprobe_probe_emits_web_page_served_with_observations() {
        let spec = builtin("builtin:webprobe");
        let text = "{\"status_code\":\"200\",\"final_url\":\"https://acme.example/\",\"title\":\"ACME Home\"}\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::WebPage,
            label: "https://acme.example/".into(),
            value: "https://acme.example/".into(),
            rel: RelationKind::Serves,
            outbound: true,
            attrs: vec![
                ("status_code".into(), "200".into(), ObsKind::Text),
                ("title".into(), "ACME Home".into(), ObsKind::Text),
                ("final_url".into(), "https://acme.example/".into(), ObsKind::Text),
            ],
        }));
    }

    #[test]
    fn hrefs_lines_link_to_discovered_web_pages() {
        let spec = builtin("builtin:hrefs");
        let text = "https://a.example/one\nhttps://a.example/two\nhttps://b.example/x\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        let pages = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::WebPage, rel: RelationKind::LinksTo, .. }))
            .count();
        assert_eq!(pages, 3, "one web page per href line");
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::WebPage,
            label: "https://b.example/x".into(),
            value: "https://b.example/x".into(),
            rel: RelationKind::LinksTo,
            outbound: true,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn webprobe_renders_the_verified_shell_pipeline() {
        let spec = builtin("builtin:webprobe");
        let args = spec
            .render_args("example.com", "example.com", "example_com", &[])
            .unwrap();
        assert_eq!(args[0], "-c");
        let expected = "b=$(curl -sL --max-time 20 --max-redirs 5 --compressed -A 'redstring probe' -w ' RSMETAxZ %{http_code} %{url_effective}' 'example.com'); meta=$(printf '%s' \"$b\" | grep -oE 'RSMETAxZ [0-9]+ [^ ]+' | tail -n1); c=$(printf '%s' \"$meta\" | cut -d' ' -f2); u=$(printf '%s' \"$meta\" | cut -d' ' -f3); case \"$c\" in ''|000) exit 0 ;; esac; t=$(printf '%s' \"$b\" | paste -sd' ' - | grep -oiE '<title[^>]*>[^<]*' | head -n1 | sed 's/<[^>]*>//'); jq -cn --arg c \"$c\" --arg u \"$u\" --arg t \"$t\" '{status_code:$c,final_url:$u,title:$t}'";
        assert_eq!(args[1], expected);
    }

    #[test]
    fn new_transforms_apply_to_declared_kinds() {
        assert!(builtin("builtin:ipapi").kinds().unwrap().contains(&EntityKind::IpAddress));
        assert!(builtin("builtin:dnsxptr").kinds().unwrap().contains(&EntityKind::IpAddress));
        assert!(builtin("builtin:subfinder").kinds().unwrap().contains(&EntityKind::Domain));
        assert!(builtin("builtin:asnmap").kinds().unwrap().contains(&EntityKind::Asn));
        assert!(builtin("builtin:gau").kinds().unwrap().contains(&EntityKind::Domain));
        let exif = builtin("builtin:exiftool").kinds().unwrap();
        assert!(exif.contains(&EntityKind::Photo));
        assert!(exif.contains(&EntityKind::Document));
        assert!(exif.contains(&EntityKind::Video));
    }

    #[test]
    fn applies_to_extensions_reach_new_kinds() {
        assert!(builtin("builtin:dnsx").kinds().unwrap().contains(&EntityKind::Server));
        assert!(builtin("builtin:tlsx").kinds().unwrap().contains(&EntityKind::Server));
        assert!(builtin("builtin:candel").kinds().unwrap().contains(&EntityKind::SocialAccount));
    }

    #[test]
    fn geocode_applies_to_location_and_address() {
        let k = builtin("builtin:geocode").kinds().unwrap();
        assert!(k.contains(&EntityKind::Location));
        assert!(k.contains(&EntityKind::Address));
    }

    #[test]
    fn geocode_json_emits_typed_coordinate_and_place() {
        let spec = builtin("builtin:geocode");
        let text = "{\"coord\":\"47.0245117,28.8322923\",\"display\":\"Chisinau, Moldova\"}";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr {
            key: "coordinates".into(),
            value: "47.0245117,28.8322923".into(),
            kind: ObsKind::Coord,
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "resolved_place".into(),
            value: "Chisinau, Moldova".into(),
            kind: ObsKind::Text,
        }));
    }

    #[test]
    fn geocode_renders_urlencoded_nominatim_pipeline() {
        let spec = builtin("builtin:geocode");
        let args = spec
            .render_args("Baia Mare, Romania", "Baia Mare, Romania", "baia_mare_romania", &[])
            .unwrap();
        assert_eq!(args[0], "-c");
        assert!(
            args[1].contains("--data-urlencode 'q=Baia Mare, Romania'"),
            "place name rides through data-urlencode: {}",
            args[1]
        );
        assert!(args[1].contains("nominatim.openstreetmap.org/search?format=json&limit=1"));
        assert!(args[1].contains("jq -c"));
    }

    #[test]
    fn favicon_applies_to_domain_and_website_and_gates_on_mmh3() {
        let spec = builtin("builtin:favicon");
        let k = spec.kinds().unwrap();
        assert!(k.contains(&EntityKind::Domain));
        assert!(k.contains(&EntityKind::Website));
        assert!(
            spec.requires.contains(&"mmh3".to_owned()),
            "favicon is gated on mmh3 so it stays hidden until mmh3 is on PATH"
        );
    }

    #[test]
    fn favicon_lines_emit_convergent_hash_pivot() {
        let spec = builtin("builtin:favicon");
        let text = "116323821\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Favicon,
            label: "116323821".into(),
            value: "116323821".into(),
            rel: RelationKind::RelatedTo,
            outbound: true,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn favicon_renders_origin_normalizing_shell() {
        let spec = builtin("builtin:favicon");
        let args = spec
            .render_args("https://evil.example/path", "evil.example", "evil_example", &[])
            .unwrap();
        assert_eq!(args[0], "-c");
        assert!(args[1].contains("v=\"https://evil.example/path\""), "value substituted: {}", args[1]);
        assert!(args[1].contains("/favicon.ico"));
        assert!(args[1].contains("mmh3.hash(base64.encodebytes(d))"));
    }

    #[test]
    fn ipapi_json_emits_geo_asn_isp_and_reverse() {
        let spec = builtin("builtin:ipapi");
        let text = "{\"status\":\"success\",\"countryCode\":\"US\",\"region\":\"CA\",\"regionName\":\"California\",\"city\":\"Mountain View\",\"isp\":\"Google LLC\",\"org\":\"Google Public DNS\",\"as\":\"AS15169 Google LLC\",\"reverse\":\"dns.google\",\"query\":\"8.8.8.8\"}";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Location,
            label: "US".into(),
            value: "US".into(),
            rel: RelationKind::LocatedAt,
            outbound: true,
            attrs: vec![
                ("city".into(), "Mountain View".into(), ObsKind::Text),
                ("region".into(), "California".into(), ObsKind::Text),
                ("country_code".into(), "US".into(), ObsKind::Text),
            ],
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Asn,
            label: "AS15169 Google LLC".into(),
            value: "AS15169 Google LLC".into(),
            rel: RelationKind::InAsn,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Organization,
            label: "Google LLC".into(),
            value: "Google LLC".into(),
            rel: RelationKind::RelatedTo,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(facts.contains(&Fact::Attr {
            key: "reverse_dns".into(),
            value: "dns.google".into(), kind: ObsKind::Text, }));
    }

    #[test]
    fn dnsxptr_ndjson_points_hostname_at_ip() {
        let spec = builtin("builtin:dnsxptr");
        let text = "{\"host\":\"8.8.8.8\",\"ttl\":74729,\"resolver\":[\"1.0.0.1:53\"],\"ptr\":[\"dns.google\"],\"status_code\":\"NOERROR\"}\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Domain,
            label: "dns.google".into(),
            value: "dns.google".into(),
            rel: RelationKind::PointsTo,
            outbound: false,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn exiftool_json_emits_author_device_date_and_gps_location() {
        let spec = builtin("builtin:exiftool");
        let text = "[{\"SourceFile\":\"evidence/docs/x.jpg\",\"Make\":\"Canon\",\"Model\":\"Canon EOS 5D\",\"Artist\":\"Jane Photographer\",\"CreateDate\":\"2021:06:15 14:23:01\",\"GPSLatitude\":\"37.774900 N\",\"GPSPosition\":\"37.774900 N, 122.419400 W\"}]";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr {
            key: "author".into(),
            value: "Jane Photographer".into(), kind: ObsKind::Text, }));
        assert!(facts.contains(&Fact::Attr {
            key: "created_at".into(),
            value: "2021:06:15 14:23:01".into(), kind: ObsKind::Text, }));
        assert!(facts.contains(&Fact::Attr {
            key: "device_model".into(),
            value: "Canon EOS 5D".into(), kind: ObsKind::Text, }));
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Location,
            label: "37.774900 N, 122.419400 W".into(),
            value: "37.774900 N, 122.419400 W".into(),
            rel: RelationKind::LocatedAt,
            outbound: true,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn email_and_crypto_transforms_apply_to_declared_kinds() {
        assert!(builtin("builtin:gleif").kinds().unwrap().contains(&EntityKind::Organization));
        assert!(builtin("builtin:hackertarget").kinds().unwrap().contains(&EntityKind::IpAddress));
        assert!(builtin("builtin:blockchain_btc").kinds().unwrap().contains(&EntityKind::CryptoWallet));
        assert!(builtin("builtin:holehe").kinds().unwrap().contains(&EntityKind::Email));
    }

    #[test]
    fn gleif_json_nested_pointers_emit_lei_status_jurisdiction() {
        let spec = builtin("builtin:gleif");
        let text = "{\"data\":[{\"type\":\"lei-records\",\"attributes\":{\"lei\":\"HWUPKR0MPOU8FGXBT394\",\"entity\":{\"legalName\":{\"name\":\"APPLE INC.\"},\"legalAddress\":{\"country\":\"US\"}},\"registration\":{\"status\":\"ISSUED\"}}}]}";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr {
            key: "lei".into(),
            value: "HWUPKR0MPOU8FGXBT394".into(), kind: ObsKind::Text, }));
        assert!(facts.contains(&Fact::Attr {
            key: "registration_status".into(),
            value: "ISSUED".into(), kind: ObsKind::Text, }));
        assert!(facts.contains(&Fact::Attr {
            key: "jurisdiction".into(),
            value: "US".into(), kind: ObsKind::Text, }));
    }

    #[test]
    fn hackertarget_lines_fan_cohosted_domains_inbound() {
        let spec = builtin("builtin:hackertarget");
        let text = "alpha.example.com\nbeta.example.com\ngamma.example.org\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        let domains = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Domain, rel: RelationKind::HostedOn, .. }))
            .count();
        assert_eq!(domains, 3, "one co-hosted domain per line");
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Domain,
            label: "beta.example.com".into(),
            value: "beta.example.com".into(),
            rel: RelationKind::HostedOn,
            outbound: false,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn blockchain_btc_json_array_fans_transactions() {
        let spec = builtin("builtin:blockchain_btc");
        let text = "{\"final_balance\":10729621465,\"n_tx\":2,\"txs\":[{\"hash\":\"8cbeef13f40de8d5e06cfd693cf2975c0a9bf7cd9191eeb435772412174509f0\"},{\"hash\":\"12b5633bad1f9c167d523ad1aa1947b2732a865bf5414eab2f9e5ae5d5c191ba\"}]}";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        let txs = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Transaction, rel: RelationKind::RelatedTo, .. }))
            .count();
        assert_eq!(txs, 2, "one transaction node per tx hash");
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Transaction,
            label: "8cbeef13f40de8d5e06cfd693cf2975c0a9bf7cd9191eeb435772412174509f0".into(),
            value: "8cbeef13f40de8d5e06cfd693cf2975c0a9bf7cd9191eeb435772412174509f0".into(),
            rel: RelationKind::RelatedTo,
            outbound: true,
            attrs: Vec::new(),
        }));
    }

    #[test]
    fn holehe_regex_named_group_emits_only_used_website_accounts() {
        let spec = builtin("builtin:holehe");
        let text = "*********************\n   target@example.com\n*********************\n[+] codepen.com\n[+] pinterest.com\n[+] wordpress.com\n[x] twitter.com\n[-] github.com\n3 websites checked\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        let sites = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Website, rel: RelationKind::UsesAccount, .. }))
            .count();
        assert_eq!(sites, 3, "only the [+] hits become accounts");
        assert!(facts.contains(&Fact::Entity {
            kind: EntityKind::Website,
            label: "codepen.com".into(),
            value: "codepen.com".into(),
            rel: RelationKind::UsesAccount,
            outbound: true,
            attrs: Vec::new(),
        }));
        assert!(
            !facts.iter().any(|f| matches!(f, Fact::Entity { value, .. } if value == "github.com" || value == "twitter.com")),
            "[-] and [x] lines must never emit an account"
        );
    }

    #[test]
    fn rotld_shape_extracts_ro_dates_and_nameservers() {
        let spec = whois_spec();
        let (shape, records) = sniff(&spec.shapes, ROTLD).unwrap().unwrap();
        assert_eq!(shape.id, "rotld", "a .ro reply must not fall through to icann");
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr { key: "registered_on".into(), value: "2000-07-17".into(), kind: ObsKind::Date }));
        assert!(facts.contains(&Fact::Entity { kind: EntityKind::Organization, label: "MarkMonitor Inc.".into(), value: "MarkMonitor Inc.".into(), rel: RelationKind::RegisteredAt, outbound: true, attrs: Vec::new() }));
        let ns = facts.iter().filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Server, rel: RelationKind::ResolvesTo, .. })).count();
        assert_eq!(ns, 2, "both .ro nameservers fan out to server nodes");
    }

    #[test]
    fn dnsx_txt_emits_dns_records_and_spf_origin_ip() {
        let spec = builtin("builtin:dnsx");
        let text = "{\"host\":\"cf.example\",\"a\":[\"104.16.1.1\"],\"txt\":[\"v=spf1 ip4:199.15.212.0 -all\",\"g=x\"],\"spf_ip4\":[\"199.15.212.0\"]}\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        let dns = facts.iter().filter(|f| matches!(f, Fact::Entity { kind: EntityKind::DnsRecord, rel: RelationKind::HasDnsRecord, .. })).count();
        assert_eq!(dns, 2, "each txt record becomes a dns_record node");
        assert!(facts.contains(&Fact::Entity { kind: EntityKind::IpAddress, label: "199.15.212.0".into(), value: "199.15.212.0".into(), rel: RelationKind::ResolvesTo, outbound: true, attrs: Vec::new() }));
    }

    #[test]
    fn wayback_lines_relate_domain_to_archived_pages() {
        let spec = builtin("builtin:wayback");
        assert!(spec.kinds().unwrap().contains(&EntityKind::Website));
        let (shape, records) = sniff(&spec.shapes, "http://a.example/\nhttps://a.example/x\n").unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        let n = facts.iter().filter(|f| matches!(f, Fact::Entity { kind: EntityKind::WebPage, rel: RelationKind::RelatedTo, .. })).count();
        assert_eq!(n, 2, "one web page per archived url line");
    }

    #[test]
    fn trackers_lines_converge_sites_on_shared_ids() {
        let spec = builtin("builtin:trackers");
        assert!(spec.kinds().unwrap().contains(&EntityKind::Domain));
        let (shape, records) = sniff(&spec.shapes, "GTM-KJZD388\nG-ABCDEF1234\nUA-12345-1\n").unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        let n = facts.iter().filter(|f| matches!(f, Fact::Entity { kind: EntityKind::TrackerId, rel: RelationKind::UsesTracker, .. })).count();
        assert_eq!(n, 3, "one tracker node per id line");
        assert!(facts.contains(&Fact::Entity { kind: EntityKind::TrackerId, label: "GTM-KJZD388".into(), value: "GTM-KJZD388".into(), rel: RelationKind::UsesTracker, outbound: true, attrs: Vec::new() }));
    }

    #[test]
    fn transforms4_apply_to_declared_kinds() {
        assert!(builtin("builtin:propublica").kinds().unwrap().contains(&EntityKind::Organization));
        let pdf = builtin("builtin:pdfforensics").kinds().unwrap();
        assert!(pdf.contains(&EntityKind::Document));
        assert!(pdf.contains(&EntityKind::CourtRecord));
        assert!(pdf.contains(&EntityKind::Photo));
        assert!(pdf.contains(&EntityKind::Video));
        let fh = builtin("builtin:filehash").kinds().unwrap();
        assert!(fh.contains(&EntityKind::Document));
        assert!(fh.contains(&EntityKind::Dataset));
    }

    #[test]
    fn propublica_json_nested_pointers_emit_financials_and_location() {
        let spec = builtin("builtin:propublica");
        let text = "{\"organization\":{\"ein\":200049703,\"name\":\"Wikimedia Foundation Org\",\"ntee_code\":\"B60\",\"state\":\"CA\",\"city\":\"San Francisco\"},\"filings_with_data\":[{\"tax_prd_yr\":2023,\"totrevenue\":180199672,\"totfuncexpns\":168305333,\"totassetsend\":274123369}]}";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr { key: "legal_name".into(), value: "Wikimedia Foundation Org".into(), kind: ObsKind::Text }));
        assert!(facts.contains(&Fact::Attr { key: "ntee".into(), value: "B60".into(), kind: ObsKind::Text }));
        assert!(facts.contains(&Fact::Attr { key: "revenue".into(), value: "180199672".into(), kind: ObsKind::Number }), "array-index pointer filings_with_data.0.totrevenue");
        assert!(facts.contains(&Fact::Attr { key: "total_expenses".into(), value: "168305333".into(), kind: ObsKind::Number }));
        assert!(facts.contains(&Fact::Attr { key: "total_assets".into(), value: "274123369".into(), kind: ObsKind::Number }));
        assert!(facts.contains(&Fact::Attr { key: "fiscal_year".into(), value: "2023".into(), kind: ObsKind::Text }));
        assert!(facts.contains(&Fact::Entity { kind: EntityKind::Location, label: "San Francisco".into(), value: "San Francisco".into(), rel: RelationKind::LocatedAt, outbound: true, attrs: vec![("region".into(), "CA".into(), ObsKind::Text)] }));
    }

    #[test]
    fn propublica_missing_990_leaves_financial_figures_absent() {
        let spec = builtin("builtin:propublica");
        let text = "{\"organization\":{\"ein\":263416850,\"name\":\"Everyday Heroes Like You Inc\",\"ntee_code\":\"P58\",\"state\":\"AZ\",\"city\":\"Scottsdale\"},\"filings_with_data\":[]}";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr { key: "ntee".into(), value: "P58".into(), kind: ObsKind::Text }));
        assert!(!facts.iter().any(|f| matches!(f, Fact::Attr { key, .. } if key == "revenue")), "empty filings_with_data yields no revenue observation");
        assert!(facts.contains(&Fact::Entity { kind: EntityKind::Location, label: "Scottsdale".into(), value: "Scottsdale".into(), rel: RelationKind::LocatedAt, outbound: true, attrs: vec![("region".into(), "AZ".into(), ObsKind::Text)] }));
    }

    #[test]
    fn pdfforensics_keyvalue_flags_mislabeled_media_and_embeds() {
        let spec = builtin("builtin:pdfforensics");
        let text = "magic_type: ISO Media, MP4 v2 [ISO 14496-14]\nembedded_count: 2\nanomaly: pdf extension but libmagic sees: ISO Media, MP4 v2 [ISO 14496-14]\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr { key: "magic_type".into(), value: "ISO Media, MP4 v2 [ISO 14496-14]".into(), kind: ObsKind::Text }));
        assert!(facts.contains(&Fact::Attr { key: "embedded_count".into(), value: "2".into(), kind: ObsKind::Text }));
        assert!(facts.contains(&Fact::Attr { key: "anomaly".into(), value: "pdf extension but libmagic sees: ISO Media, MP4 v2 [ISO 14496-14]".into(), kind: ObsKind::Text }), "first-colon split keeps the colon inside the anomaly note");
    }

    #[test]
    fn filehash_regex_lifts_sha256_digest() {
        let spec = builtin("builtin:filehash");
        let text = "df257009b71f175a22e3023f79908078345cdef341f1b4fe58811dcc8ebb8c0c  evidence/docs/real.pdf\n";
        let (shape, records) = sniff(&spec.shapes, text).unwrap().unwrap();
        let facts = emit(shape, &records).unwrap();
        assert!(facts.contains(&Fact::Attr { key: "sha256".into(), value: "df257009b71f175a22e3023f79908078345cdef341f1b4fe58811dcc8ebb8c0c".into(), kind: ObsKind::Text }));
    }

    #[test]
    fn filehash_renders_sha256sum_over_project_relative_file() {
        let spec = builtin("builtin:filehash");
        let args = spec.render_args("", "real.pdf", "real_pdf", &[("file".into(), "evidence/docs/real.pdf".into())]).unwrap();
        assert_eq!(args[0], "-c");
        assert_eq!(args[1], "sha256sum \"evidence/docs/real.pdf\"");
    }
}
