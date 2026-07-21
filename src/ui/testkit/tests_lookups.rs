#[cfg(test)]
mod tests {
    use crate::model::{EntityKind, Origin, RelationKind, Verification};
    use crate::modules::{Fact, ObsKind};
    use crate::ui::testkit::{apply, fixtures, graph, harness, invariant};

    fn ent(
        kind: EntityKind,
        label: &str,
        value: &str,
        rel: RelationKind,
        attrs: Vec<(String, String)>,
    ) -> Fact {
        Fact::Entity {
            kind,
            label: label.to_owned(),
            value: value.to_owned(),
            rel,
            outbound: true,
            attrs: attrs
                .into_iter()
                .map(|(k, v)| (k, v, ObsKind::Text))
                .collect(),
        }
    }

    fn attr(key: &str, value: &str) -> Fact {
        attrk(key, value, ObsKind::Text)
    }

    fn attrk(key: &str, value: &str, kind: ObsKind) -> Fact {
        Fact::Attr {
            key: key.to_owned(),
            value: value.to_owned(),
            kind,
        }
    }

    fn parse(module: &str, fixture: &str) -> (String, Vec<Fact>) {
        let ron = fixtures::module_ron(module).unwrap();
        let raw = fixtures::fixture(fixture).unwrap();
        apply::parse(&ron, &raw).unwrap()
    }

    #[test]
    fn whois_domain_sniffs_icann_and_emits_attrs_org_and_nameservers() {
        let (shape, facts) = parse("whois", "whois-domain.txt");
        assert_eq!(shape, "icann");
        assert_eq!(facts.len(), 7, "4 attrs + 1 org + 2 nameservers");
        assert!(facts.contains(&attr("registrar", "MarkMonitor Inc.")));
        assert!(facts.contains(&attrk("registered_on", "2019-04-12T10:22:31Z", ObsKind::Date)));
        assert!(facts.contains(&attrk("expires", "2025-04-12T10:22:31Z", ObsKind::Date)));
        assert!(facts.contains(&attr("dnssec", "unsigned")));
        assert!(facts.contains(&ent(
            EntityKind::Organization,
            "MarkMonitor Inc.",
            "MarkMonitor Inc.",
            RelationKind::RegisteredAt,
            Vec::new(),
        )));
        assert!(facts.contains(&ent(
            EntityKind::Server,
            "NS1.EXAMPLE-DNS.COM",
            "NS1.EXAMPLE-DNS.COM",
            RelationKind::ResolvesTo,
            Vec::new(),
        )));
        assert!(facts.contains(&ent(
            EntityKind::Server,
            "NS2.EXAMPLE-DNS.COM",
            "NS2.EXAMPLE-DNS.COM",
            RelationKind::ResolvesTo,
            Vec::new(),
        )));
    }

    #[test]
    fn whois_ip_sniffs_arin_and_coalesces_geo_into_one_location() {
        let (shape, facts) = parse("whois", "whois-ip.txt");
        assert_eq!(shape, "arin");
        assert!(facts.contains(&attr("range", "203.0.113.0 - 203.0.113.255")));
        assert!(facts.contains(&attr("cidr", "203.0.113.0/24")));
        assert!(facts.contains(&ent(
            EntityKind::Organization,
            "Evil Corp Hosting LLC",
            "Evil Corp Hosting LLC",
            RelationKind::RegisteredAt,
            Vec::new(),
        )));
        assert!(facts.contains(&ent(
            EntityKind::Location,
            "US",
            "US",
            RelationKind::LocatedAt,
            vec![
                ("country_code".to_owned(), "US".to_owned()),
                ("city".to_owned(), "Reno".to_owned()),
                ("region".to_owned(), "NV".to_owned()),
            ],
        )));
        let locations = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Location, .. }))
            .count();
        assert_eq!(locations, 1, "geo vars coalesce into a single location node");
    }

    #[test]
    fn tlsx_emits_one_certificate_and_a_domain_per_san() {
        let (shape, facts) = parse("tlsx", "tlsx.jsonl");
        assert_eq!(shape, "tlsx");
        assert_eq!(facts.len(), 4, "1 cert + 3 SAN domains");
        assert!(facts.contains(&ent(
            EntityKind::TlsCertificate,
            "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678abcdef0123456789abcdef01",
            "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678abcdef0123456789abcdef01",
            RelationKind::PresentsCert,
            vec![
                (
                    "sha256".to_owned(),
                    "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678abcdef0123456789abcdef01".to_owned(),
                ),
                ("issuer".to_owned(), "R3".to_owned()),
                ("subject".to_owned(), "evil-corp.example".to_owned()),
            ],
        )));
        for san in ["evil-corp.example", "www.evil-corp.example", "api.evil-corp.example"] {
            assert!(
                facts.contains(&ent(
                    EntityKind::Domain,
                    san,
                    san,
                    RelationKind::Covers,
                    Vec::new(),
                )),
                "missing covered domain {san}"
            );
        }
    }

    #[test]
    fn crtsh_emits_a_domain_and_issuer_attr_per_record() {
        let (shape, facts) = parse("crtsh", "crtsh.jsonl");
        assert_eq!(shape, "crtsh");
        let domains = facts
            .iter()
            .filter(|f| matches!(f, Fact::Entity { kind: EntityKind::Domain, .. }))
            .count();
        assert_eq!(domains, 4, "one domain per CT log SAN");
        assert!(facts.contains(&attr("ct_issuer", "C=US, O=Let's Encrypt, CN=R3")));
        for san in [
            "evil-corp.example",
            "mail.evil-corp.example",
            "vpn.evil-corp.example",
            "staging.evil-corp.example",
        ] {
            assert!(
                facts.contains(&ent(
                    EntityKind::Domain,
                    san,
                    san,
                    RelationKind::Covers,
                    Vec::new()
                )),
                "missing {san}"
            );
        }
    }

    #[test]
    fn crtsh_flaky_html_body_degrades_to_zero_facts() {
        let (shape, facts) = parse("crtsh", "crtsh-404.html");
        assert_eq!(shape, "", "no shape sniffs an HTML error body");
        assert!(facts.is_empty(), "HTML garbage yields no facts, not junk");
    }

    #[test]
    fn katana_emits_a_web_page_per_crawled_url() {
        let (shape, facts) = parse("katana", "katana.jsonl");
        assert_eq!(shape, "katana");
        assert_eq!(facts.len(), 4);
        assert!(facts.contains(&ent(
            EntityKind::WebPage,
            "https://evil-corp.example/login",
            "https://evil-corp.example/login",
            RelationKind::LinksTo,
            Vec::new(),
        )));
    }

    #[test]
    fn dnsx_fans_records_to_ips_and_servers() {
        let (shape, facts) = parse("dnsx", "dnsx.jsonl");
        assert_eq!(shape, "dnsx");
        assert_eq!(facts.len(), 5, "2 A + 2 NS + 1 MX");
        assert!(facts.contains(&ent(
            EntityKind::IpAddress,
            "203.0.113.10",
            "203.0.113.10",
            RelationKind::ResolvesTo,
            Vec::new(),
        )));
        assert!(facts.contains(&ent(
            EntityKind::Server,
            "ns1.example-dns.com",
            "ns1.example-dns.com",
            RelationKind::ResolvesTo,
            Vec::new(),
        )));
        assert!(facts.contains(&ent(
            EntityKind::Server,
            "mail.evil-corp.example",
            "mail.evil-corp.example",
            RelationKind::PointsTo,
            Vec::new(),
        )));
    }

    #[test]
    fn candel_labels_websites_by_host_keeps_url_as_value() {
        let (shape, facts) = parse("candel", "candel.jsonl");
        assert_eq!(shape, "candel");
        assert_eq!(facts.len(), 3);
        assert!(facts.contains(&ent(
            EntityKind::Website,
            "github.com",
            "https://github.com/evilcorp",
            RelationKind::UsesAccount,
            Vec::new(),
        )));
        assert!(facts.contains(&ent(
            EntityKind::Website,
            "reddit.com",
            "https://www.reddit.com/user/evilcorp",
            RelationKind::UsesAccount,
            Vec::new(),
        )));
    }

    #[test]
    fn every_builtin_module_id_has_a_loadable_spec() {
        for id in fixtures::MODULE_IDS {
            let ron = fixtures::module_ron(id).unwrap();
            assert!(
                ron.contains(&format!("id: \"{id}\"")),
                "module {id} ron must self-identify"
            );
        }
        assert_eq!(fixtures::MODULE_IDS.len(), 6);
    }

    #[test]
    fn every_builtin_module_fixture_extracts_nonempty_facts() {
        let cases = [
            ("whois", "whois-domain.txt"),
            ("whois", "whois-ip.txt"),
            ("tlsx", "tlsx.jsonl"),
            ("crtsh", "crtsh.jsonl"),
            ("katana", "katana.jsonl"),
            ("dnsx", "dnsx.jsonl"),
            ("candel", "candel.jsonl"),
        ];
        for (module, fixture) in cases {
            let (shape, facts) = parse(module, fixture);
            assert!(!shape.is_empty(), "{module}/{fixture} sniffed no shape");
            assert!(!facts.is_empty(), "{module}/{fixture} emitted no facts");
        }
    }

    #[test]
    fn apply_dnsx_spawns_ip_and_server_nodes_with_sourced_edges() {
        let mut app = harness::app().unwrap();
        let dom = graph::add_valued(
            &mut app,
            EntityKind::Domain,
            "evil-corp.example",
            "evil-corp.example",
        )
        .unwrap();
        let raw = fixtures::fixture("dnsx.jsonl").unwrap();
        let ron = fixtures::module_ron("dnsx").unwrap();
        let outcome = apply::apply_to(&mut app, &ron, "dnsx", dom, &raw).unwrap();
        assert_eq!(outcome.nodes_before, 1);
        assert_eq!(outcome.nodes_after, 6, "1 domain + 2 IPs + 3 servers");
        assert_eq!(outcome.edges_before, 0);
        assert_eq!(outcome.edges_after, 5);
        assert!(!outcome.status.is_empty(), "apply reports a status line");

        let ip = graph::node_by_slug(&app, "203_0_113_10").unwrap().unwrap();
        assert_eq!(graph::kind_of(&app, ip).unwrap(), EntityKind::IpAddress);
        assert!(graph::has_edge(&app, dom, ip, RelationKind::ResolvesTo).unwrap());
        let srcs = graph::edge_sources(&app, dom, ip, RelationKind::ResolvesTo).unwrap();
        assert!(!srcs.is_empty(), "every lookup edge carries a source");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn apply_whois_domain_stamps_machine_observations_and_links_registrar() {
        let mut app = harness::app().unwrap();
        let dom = graph::add_valued(
            &mut app,
            EntityKind::Domain,
            "evil-corp.example",
            "evil-corp.example",
        )
        .unwrap();
        let raw = fixtures::fixture("whois-domain.txt").unwrap();
        let ron = fixtures::module_ron("whois").unwrap();
        apply::apply_to(&mut app, &ron, "whois", dom, &raw).unwrap();

        let reg = graph::observation(&app, dom, "registrar").unwrap().unwrap();
        assert_eq!(reg.value.text().unwrap(), "MarkMonitor Inc.");
        assert_eq!(reg.origin, Origin::Lookup("whois".to_owned()));
        assert_eq!(reg.status, Verification::MachineSuggested);
        assert!(!reg.author.name.is_empty());

        let org = graph::node_by_slug(&app, "markmonitor_inc").unwrap().unwrap();
        assert_eq!(graph::kind_of(&app, org).unwrap(), EntityKind::Organization);
        assert!(graph::has_edge(&app, dom, org, RelationKind::RegisteredAt).unwrap());
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn apply_whois_ip_creates_single_location_node() {
        let mut app = harness::app().unwrap();
        let ip = graph::add_valued(&mut app, EntityKind::IpAddress, "203.0.113.10", "203.0.113.10")
            .unwrap();
        let raw = fixtures::fixture("whois-ip.txt").unwrap();
        let ron = fixtures::module_ron("whois").unwrap();
        apply::apply_to(&mut app, &ron, "whois", ip, &raw).unwrap();

        let loc = graph::node_by_slug(&app, "us").unwrap().unwrap();
        assert_eq!(graph::kind_of(&app, loc).unwrap(), EntityKind::Location);
        assert_eq!(
            graph::observation(&app, loc, "city")
                .unwrap()
                .unwrap()
                .value
                .text()
                .unwrap(),
            "Reno"
        );
        assert!(graph::has_edge(&app, ip, loc, RelationKind::LocatedAt).unwrap());
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn apply_katana_links_every_crawled_page() {
        let mut app = harness::app().unwrap();
        let dom = graph::add_valued(
            &mut app,
            EntityKind::Domain,
            "evil-corp.example",
            "evil-corp.example",
        )
        .unwrap();
        let raw = fixtures::fixture("katana.jsonl").unwrap();
        let ron = fixtures::module_ron("katana").unwrap();
        let outcome = apply::apply_to(&mut app, &ron, "katana", dom, &raw).unwrap();
        assert_eq!(outcome.nodes_after, 5, "1 domain + 4 web pages");
        assert_eq!(outcome.edges_after, 4);
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn apply_html_degrade_creates_no_nodes() {
        let mut app = harness::app().unwrap();
        let dom = graph::add_valued(
            &mut app,
            EntityKind::Domain,
            "evil-corp.example",
            "evil-corp.example",
        )
        .unwrap();
        let raw = fixtures::fixture("crtsh-404.html").unwrap();
        let ron = fixtures::module_ron("crtsh").unwrap();
        let outcome = apply::apply_to(&mut app, &ron, "crtsh", dom, &raw).unwrap();
        assert_eq!(outcome.shape_id, "");
        assert!(outcome.facts.is_empty());
        assert_eq!(outcome.nodes_after, 1, "degrade adds no nodes");
        assert_eq!(outcome.edges_after, 0);
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn applying_the_same_lookup_twice_is_idempotent() {
        let mut app = harness::app().unwrap();
        let dom = graph::add_valued(
            &mut app,
            EntityKind::Domain,
            "evil-corp.example",
            "evil-corp.example",
        )
        .unwrap();
        let raw = fixtures::fixture("dnsx.jsonl").unwrap();
        let ron = fixtures::module_ron("dnsx").unwrap();
        apply::apply_to(&mut app, &ron, "dnsx", dom, &raw).unwrap();
        let nodes = graph::node_count(&app).unwrap();
        let edges = graph::edge_count(&app).unwrap();
        let second = apply::apply_to(&mut app, &ron, "dnsx", dom, &raw).unwrap();
        assert_eq!(second.nodes_after, nodes, "re-apply spawns no duplicate nodes");
        assert_eq!(second.edges_after, edges, "re-apply adds no duplicate edges");
        assert_eq!(
            second.edges_before, second.edges_after,
            "re-apply is a no-op on the edge set"
        );
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn lookup_converges_onto_an_aligned_existing_node() {
        let mut app = harness::app().unwrap();
        let dom = graph::add_valued(
            &mut app,
            EntityKind::Domain,
            "evil-corp.example",
            "evil-corp.example",
        )
        .unwrap();
        let existing_ip =
            graph::add_valued(&mut app, EntityKind::IpAddress, "203.0.113.10", "203.0.113.10")
                .unwrap();
        let before = graph::node_count(&app).unwrap();
        let raw = fixtures::fixture("dnsx.jsonl").unwrap();
        let ron = fixtures::module_ron("dnsx").unwrap();
        apply::apply_to(&mut app, &ron, "dnsx", dom, &raw).unwrap();
        let hits = graph::nodes_with_value(&app, EntityKind::IpAddress, "203.0.113.10").unwrap();
        assert_eq!(hits.len(), 1, "emitted IP converges onto the pre-existing node");
        assert_eq!(hits[0], existing_ip);
        assert_eq!(
            graph::node_count(&app).unwrap(),
            before + 4,
            "only the 4 genuinely new nodes spawn (second IP + two NS + one MX)"
        );
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn lookup_converges_despite_mismatched_label_slug() {
        let mut app = harness::app().unwrap();
        let dom = graph::add(&mut app, EntityKind::Domain, "Evil Corp Primary Site").unwrap();
        graph::set_value(&mut app, dom, "evil-corp.example").unwrap();
        let raw = fixtures::fixture("crtsh.jsonl").unwrap();
        let ron = fixtures::module_ron("crtsh").unwrap();
        apply::apply_to(&mut app, &ron, "crtsh", dom, &raw).unwrap();
        let dups = graph::nodes_with_value(&app, EntityKind::Domain, "evil-corp.example").unwrap();
        assert_eq!(
            dups.len(),
            1,
            "the emitted domain must converge onto the existing node, not duplicate"
        );
        invariant::check_graph(&app).unwrap();
    }
}
