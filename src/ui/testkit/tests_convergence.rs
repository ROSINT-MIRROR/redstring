#[cfg(test)]
mod tests {
    use crate::model::{EntityKind, Origin, RelationKind, Verification};
    use crate::project::Project;
    use crate::ui::RedstringApp;
    use crate::ui::testkit::{apply, fixtures, graph, harness, invariant};

    fn count_with_slug(app: &RedstringApp, slug: &str) -> usize {
        let g = app.g.g();
        let mut c = 0usize;
        for idx in g.node_indices() {
            let Some(n) = g.node_weight(idx) else {
                continue;
            };
            if n.payload().slug == slug {
                c += 1;
            }
        }
        c
    }

    fn run(app: &mut RedstringApp, module: &str, target: petgraph::graph::NodeIndex, fixture: &str) {
        let ron = fixtures::module_ron(module).unwrap();
        let raw = fixtures::fixture(fixture).unwrap();
        apply::apply_to(app, &ron, module, target, &raw).unwrap();
    }

    fn domain(app: &mut RedstringApp) -> petgraph::graph::NodeIndex {
        graph::add_valued(app, EntityKind::Domain, "evil-corp.example", "evil-corp.example").unwrap()
    }

    #[test]
    fn two_modules_converge_on_shared_nameservers_without_duplicating() {
        let mut app = harness::app().unwrap();
        let dom = domain(&mut app);
        run(&mut app, "dnsx", dom, "dnsx.jsonl");
        assert_eq!(graph::node_count(&app).unwrap(), 6, "dnsx spawns 2 IPs + 2 NS + 1 MX");
        run(&mut app, "whois", dom, "whois-domain.txt");

        assert_eq!(
            graph::node_count(&app).unwrap(),
            7,
            "only the registrar org is genuinely new; both nameservers converge"
        );
        assert_eq!(count_with_slug(&app, "ns1_example_dns_com"), 1, "NS1 must not duplicate ns1");
        assert_eq!(count_with_slug(&app, "ns2_example_dns_com"), 1, "NS2 must not duplicate ns2");
        assert!(graph::node_by_slug(&app, "markmonitor_inc").unwrap().is_some());
        let reg = graph::observation(&app, dom, "registrar").unwrap().unwrap();
        assert_eq!(reg.value.text().unwrap(), "MarkMonitor Inc.");
        assert_eq!(reg.origin, Origin::Lookup("whois".to_owned()));
        assert_eq!(reg.status, Verification::MachineSuggested);
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn multi_hop_domain_to_ip_to_org_and_location() {
        let mut app = harness::app().unwrap();
        let dom = domain(&mut app);
        run(&mut app, "dnsx", dom, "dnsx.jsonl");
        let ip = graph::node_by_slug(&app, "203_0_113_10").unwrap().unwrap();
        assert_eq!(graph::kind_of(&app, ip).unwrap(), EntityKind::IpAddress);

        run(&mut app, "whois", ip, "whois-ip.txt");
        assert_eq!(graph::node_count(&app).unwrap(), 8, "the IP's whois adds one org and one location");
        let org = graph::node_by_slug(&app, "evil_corp_hosting_llc").unwrap().unwrap();
        let loc = graph::node_by_slug(&app, "us").unwrap().unwrap();

        assert!(graph::has_edge(&app, dom, ip, RelationKind::ResolvesTo).unwrap(), "hop one: domain -> ip");
        assert!(graph::has_edge(&app, ip, org, RelationKind::RegisteredAt).unwrap(), "hop two: ip -> org");
        assert!(graph::has_edge(&app, ip, loc, RelationKind::LocatedAt).unwrap(), "hop two: ip -> location");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn a_manual_node_absorbs_a_later_cross_module_emit() {
        let mut app = harness::app().unwrap();
        let dom = domain(&mut app);
        let manual =
            graph::add_valued(&mut app, EntityKind::Server, "ns1.example-dns.com", "ns1.example-dns.com")
                .unwrap();
        run(&mut app, "whois", dom, "whois-domain.txt");
        assert_eq!(
            count_with_slug(&app, "ns1_example_dns_com"),
            1,
            "the whois NS1 emit converges onto the manually created server"
        );
        let converged = graph::node_by_slug(&app, "ns1_example_dns_com").unwrap().unwrap();
        assert_eq!(converged, manual, "convergence reuses the existing node index");
        assert!(graph::has_edge(&app, dom, converged, RelationKind::ResolvesTo).unwrap());
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn repeated_multi_module_application_is_idempotent() {
        let mut app = harness::app().unwrap();
        let dom = domain(&mut app);
        run(&mut app, "dnsx", dom, "dnsx.jsonl");
        run(&mut app, "whois", dom, "whois-domain.txt");
        let nodes = graph::node_count(&app).unwrap();
        let edges = graph::edge_count(&app).unwrap();
        for _ in 0..3 {
            run(&mut app, "dnsx", dom, "dnsx.jsonl");
            run(&mut app, "whois", dom, "whois-domain.txt");
        }
        assert_eq!(graph::node_count(&app).unwrap(), nodes, "re-running modules spawns no duplicates");
        assert_eq!(graph::edge_count(&app).unwrap(), edges, "re-running modules adds no duplicate edges");
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn a_convergence_chain_saves_reloads_and_validates_clean() {
        let (td, project) = harness::temp_project().unwrap();
        let root = project.root.clone();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let dom = domain(&mut app);
        run(&mut app, "dnsx", dom, "dnsx.jsonl");
        run(&mut app, "whois", dom, "whois-domain.txt");
        let ip = graph::node_by_slug(&app, "203_0_113_10").unwrap().unwrap();
        run(&mut app, "whois", ip, "whois-ip.txt");
        let before_nodes = graph::node_count(&app).unwrap();
        let before_slugs = graph::slugs(&app).unwrap();

        harness::save(&mut app).unwrap();
        invariant::check_case_dir(&Project::open(&root).unwrap()).unwrap();

        let mut app2 = harness::app().unwrap();
        app2.project = Some(Project::open(&root).unwrap());
        harness::load(&mut app2).unwrap();
        assert_eq!(graph::node_count(&app2).unwrap(), before_nodes, "the chain round-trips node-for-node");
        assert_eq!(graph::slugs(&app2).unwrap(), before_slugs, "every slug survives save and reload");
        invariant::check_graph(&app2).unwrap();
        drop(td);
    }
}
