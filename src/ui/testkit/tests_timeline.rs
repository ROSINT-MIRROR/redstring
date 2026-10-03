#[cfg(test)]
mod tests {
    use petgraph::graph::NodeIndex;

    use crate::model::{
        DatePrecision, DateVal, EntityKind, Origin, RelationKind, Source, SourceKind, SourceTier,
        Verification,
    };
    use crate::ui::RedstringApp;
    use crate::ui::testkit::{fixtures, graph, harness, invariant};
    use crate::ui::timeline::derive::Digest;
    use crate::ui::timeline::event::{Footing, Lane, Record, TimelineEvent};
    use crate::ui::timeline::state::Fit;
    use crate::ui::timeline::when::{Reading, When, Zone, read_text_date};

    fn demo_app() -> (harness::TempDir, RedstringApp) {
        let td = harness::temp_dir().unwrap();
        let project = fixtures::open_timeline_case(&td.path.join("case")).unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        harness::load(&mut app).unwrap();
        (td, app)
    }

    fn node(app: &RedstringApp, slug: &str) -> NodeIndex {
        app.g
            .g()
            .node_indices()
            .find(|i| app.g.node(*i).unwrap().payload().slug == slug)
            .unwrap_or_else(|| panic!("no node {slug}"))
    }

    fn on_lane(d: &Digest, lane: Lane) -> Vec<&TimelineEvent> {
        d.events.iter().filter(|e| e.key.lane == lane).collect()
    }

    fn event<'a>(d: &'a Digest, lane: Lane, title_part: &str) -> &'a TimelineEvent {
        d.events
            .iter()
            .find(|e| e.key.lane == lane && e.title.contains(title_part))
            .unwrap_or_else(|| panic!("no {lane:?} mark containing '{title_part}'"))
    }

    fn date(secs: i64, precision: DatePrecision) -> When {
        When::Date(DateVal { secs, precision })
    }

    #[test]
    fn text_dates_are_read_strictly_and_keep_their_stated_zone() {
        let Reading::Dated(utc) = read_text_date("2018-02-27T10:22:31Z").unwrap() else {
            panic!("an RFC 3339 UTC stamp is a date")
        };
        assert_eq!(
            utc,
            When::Instant {
                secs: 1_519_726_951,
                zone: Zone::Utc
            }
        );
        assert_eq!(utc.label().unwrap(), "2018-02-27 10:22 UTC");

        let Reading::Dated(offset) = read_text_date("2019-04-11T22:47:00+03:00").unwrap() else {
            panic!("an offset stamp is a date")
        };
        assert_eq!(
            offset,
            When::Instant {
                secs: 1_555_012_020,
                zone: Zone::OffsetMinutes(180)
            }
        );
        assert_eq!(
            offset.label().unwrap(),
            "2019-04-11 22:47 +03:00",
            "wall clock as stated"
        );

        let Reading::Dated(naive) = read_text_date("2019-05-03 14:30").unwrap() else {
            panic!("a zone-less stamp is still a date")
        };
        assert_eq!(
            naive,
            When::Instant {
                secs: 1_556_893_800,
                zone: Zone::Unstated
            }
        );
        let b = naive.bounds().unwrap();
        assert_eq!(
            (b.start, b.end),
            (1_556_841_600, 1_556_928_000),
            "no zone → the whole day"
        );
        assert!(naive.note().unwrap().contains("without a zone"));

        let Reading::Dated(day) = read_text_date("2019-04-12").unwrap() else {
            panic!("a bare ISO day is a date")
        };
        assert_eq!(day, date(1_555_027_200, DatePrecision::Day));

        for not_a_date in [
            "2019",
            "2019-04",
            "2019-02-31",
            "12345678901",
            "registrar: X",
            "",
            "2019-13-01",
        ] {
            assert_eq!(
                read_text_date(not_a_date).unwrap(),
                Reading::NotADate,
                "'{not_a_date}' must not be promoted to a date"
            );
        }
    }

    #[test]
    fn a_coarse_date_spans_its_whole_period_and_is_never_a_point() {
        let month = date(1_519_862_400, DatePrecision::Month).bounds().unwrap();
        assert_eq!(
            (month.start, month.end),
            (1_519_862_400, 1_522_540_800),
            "all of March 2018"
        );
        let year = date(1_483_228_800, DatePrecision::Year).bounds().unwrap();
        assert_eq!(
            (year.start, year.end),
            (1_483_228_800, 1_514_764_800),
            "all of 2017"
        );
        let decade = date(1_483_228_800, DatePrecision::Decade).bounds().unwrap();
        assert_eq!(
            (decade.start, decade.end),
            (1_262_304_000, 1_577_836_800),
            "the 2010s"
        );
        let circa = date(1_483_228_800, DatePrecision::Circa);
        assert!(circa.note().unwrap().contains("approximate"));
        assert_eq!(circa.label().unwrap(), "~2017");
    }

    #[test]
    fn footing_never_promotes_a_hypothesis_or_an_unsourced_claim() {
        let src = |reference: &str, tier: SourceTier| Source {
            kind: SourceKind::Url,
            reference: reference.to_owned(),
            tier,
            ..Default::default()
        };
        let one = [src("https://a.example", SourceTier::Primary)];
        let two = [
            one[0].clone(),
            src("https://b.example", SourceTier::Secondary),
        ];
        let three = [
            two[0].clone(),
            two[1].clone(),
            src("https://c.example", SourceTier::Reported),
        ];
        let inferred = Footing::of(Verification::Corroborated, &Origin::Inference, &three).unwrap();
        assert_eq!(
            inferred,
            Footing::Unproven,
            "an inference stays a hypothesis whatever its tag"
        );
        let bare = Footing::of(Verification::Corroborated, &Origin::Manual, &[]).unwrap();
        assert_eq!(bare, Footing::Unproven, "no source, no fact");
        let asserted = Footing::of(Verification::AnalystAsserted, &Origin::Manual, &one).unwrap();
        assert_eq!(
            asserted,
            Footing::Sourced,
            "a primary tier alone confirms nothing"
        );
        let confirmed = Footing::of(Verification::Confirmed, &Origin::Manual, &one).unwrap();
        assert_eq!(confirmed, Footing::Confirmed);
        let lone = Footing::of(Verification::Corroborated, &Origin::Manual, &one).unwrap();
        assert_eq!(lone, Footing::Sourced, "one origin is not corroboration");
        let solid = Footing::of(Verification::Corroborated, &Origin::Manual, &two).unwrap();
        assert_eq!(solid, Footing::Corroborated);
        let denied = Footing::of(Verification::DeniedByTarget, &Origin::Manual, &one).unwrap();
        assert_eq!(denied, Footing::Disputed);
        let gone = Footing::of(Verification::NoPublicRecord, &Origin::Manual, &one).unwrap();
        assert_eq!(gone, Footing::Disputed);
        let rejected = Footing::of(Verification::Rejected, &Origin::Manual, &three).unwrap();
        assert_eq!(rejected, Footing::Rejected);
    }

    #[test]
    fn the_demo_case_projects_onto_four_distinct_times() {
        let (_td, app) = demo_app();
        let d = Digest::of(&app.g).unwrap();
        assert_eq!(
            on_lane(&d, Lane::Event).len(),
            13,
            "8 dated facts + 5 dated relations"
        );
        assert_eq!(on_lane(&d, Lane::Published).len(), 7);
        assert_eq!(on_lane(&d, Lane::Captured).len(), 12);
        assert_eq!(on_lane(&d, Lane::Entered).len(), 15);
        assert_eq!(
            d.unknown_entry, 1,
            "the unstamped relation is not given an entry time"
        );

        let appointed = event(&d, Lane::Event, "Appointed Deputy Mayor");
        let published = event(&d, Lane::Published, "Appointed Deputy Mayor");
        let captured = event(&d, Lane::Captured, "Appointed Deputy Mayor");
        let entered = event(&d, Lane::Entered, "Appointed Deputy Mayor");
        assert_eq!(appointed.when.label().unwrap(), "2016-06-12", "event time");
        assert_eq!(
            published.when.label().unwrap(),
            "2016-06-20",
            "publication time"
        );
        assert_eq!(
            captured.when.label().unwrap(),
            "2026-08-14 09:12 UTC",
            "capture time"
        );
        assert_eq!(
            entered.when.label().unwrap(),
            "2026-08-14 10:02 UTC",
            "case-entry time"
        );
        let record = appointed.key.record;
        for other in [published, captured, entered] {
            assert_eq!(
                other.key.record, record,
                "all four times hang off one case record"
            );
        }
    }

    #[test]
    fn the_demo_keeps_uncertainty_zones_and_epistemic_status() {
        let (_td, app) = demo_app();
        let d = Digest::of(&app.g).unwrap();

        let email = event(&d, Lane::Event, "contract ready");
        assert_eq!(
            email.when,
            When::Instant {
                secs: 1_555_012_020,
                zone: Zone::OffsetMinutes(180)
            }
        );
        assert!(
            email.read_from_text,
            "the stamp was read from the record's text, not invented"
        );

        let visit = event(&d, Lane::Event, "Site visit");
        assert!(matches!(
            visit.when,
            When::Instant {
                zone: Zone::Unstated,
                ..
            }
        ));

        let dinner = event(&d, Lane::Event, "Dinner at Hotel Central");
        assert_eq!(
            dinner.footing,
            Footing::Unproven,
            "inferred and unsourced: a hypothesis"
        );
        assert_eq!(
            dinner.when.label().unwrap(),
            "2019-02",
            "month precision is kept, not sharpened"
        );

        let owner = event(&d, Lane::Event, "beneficial owner of");
        assert!(
            matches!(owner.when, When::From(..)),
            "an open-ended period stays open"
        );
        assert!(owner.span.open_end);
        assert_eq!(owner.footing, Footing::Unproven);

        assert_eq!(
            event(&d, Lane::Event, "Town Hall — sued").footing,
            Footing::Rejected
        );
        assert_eq!(
            event(&d, Lane::Event, "Paving SRL — paid").footing,
            Footing::Disputed
        );
        assert_eq!(
            event(&d, Lane::Event, "Vale — director of").footing,
            Footing::Confirmed,
            "one primary registry record confirms the directorship; it does not corroborate it"
        );
        assert_eq!(
            event(&d, Lane::Event, "Appointed Deputy Mayor").footing,
            Footing::Confirmed
        );
        assert_eq!(
            event(&d, Lane::Event, "Tender").footing,
            Footing::Corroborated,
            "the procurement portal and the FOI release are two independent origins"
        );
    }

    #[test]
    fn conflicting_accounts_are_both_kept_and_linked() {
        let (_td, app) = demo_app();
        let d = Digest::of(&app.g).unwrap();
        assert_eq!(d.conflict_count().unwrap(), 1);
        let clashing: Vec<&TimelineEvent> = d
            .events
            .iter()
            .filter(|e| !e.conflicts.is_empty())
            .collect();
        assert_eq!(clashing.len(), 2);
        let mut labels: Vec<String> = clashing.iter().map(|e| e.when.label().unwrap()).collect();
        labels.sort();
        assert_eq!(labels, vec!["2017".to_owned(), "2018-03".to_owned()]);
        for e in &clashing {
            assert!(e.title.contains("Incorporated"));
            let other = &d.events[e.conflicts[0]];
            assert_ne!(
                other.key.record, e.key.record,
                "each account is its own record"
            );
        }
    }

    #[test]
    fn records_without_any_date_are_listed_never_placed() {
        let (_td, app) = demo_app();
        let d = Digest::of(&app.g).unwrap();
        let titles: Vec<&str> = d.unplaced.iter().map(|u| u.title.as_str()).collect();
        assert_eq!(titles.len(), 2, "{titles:?}");
        assert!(titles.iter().any(|t| t.contains("Meeting with the mayor")));
        assert!(titles.iter().any(|t| t.contains("associate of")));
        let mayor = node(&app, "meeting_with_the_mayor");
        assert!(
            d.events
                .iter()
                .all(|e| e.key.record != Record::Obs(mayor, 0)),
            "an undated meeting gets no mark on any lane"
        );
    }

    #[test]
    fn no_time_is_invented_for_text_attributes_or_unstamped_records() {
        let mut app = harness::app().unwrap();
        let org = graph::add(&mut app, EntityKind::Organization, "Quiet Co").unwrap();
        let person = graph::add(&mut app, EntityKind::Person, "A. Person").unwrap();
        graph::observe(
            &mut app,
            org,
            "registrar",
            "Example Inc.",
            Origin::Manual,
            Verification::AnalystAsserted,
        )
        .unwrap();
        graph::observe(
            &mut app,
            org,
            "founded",
            "sometime in the nineties",
            Origin::Manual,
            Verification::Unreviewed,
        )
        .unwrap();
        let edge = graph::link(&mut app, person, org, RelationKind::Controls).unwrap();
        app.g.edge_mut(edge).unwrap().payload_mut().created_at = 0;

        let d = Digest::of(&app.g).unwrap();
        assert!(
            d.events.is_empty(),
            "nothing here states a time: {:?}",
            d.events
        );
        assert_eq!(d.unplaced.len(), 1, "the relation is listed as unplaced");
        assert_eq!(
            d.unknown_entry, 0,
            "an unplaced record is not counted as placed"
        );
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn a_mark_navigates_to_its_record_on_the_board() {
        let (_td, mut app) = demo_app();
        let d = Digest::of(&app.g).unwrap();
        let email = event(&d, Lane::Event, "contract ready").key.record;
        harness::frame(&mut app, |a, ui| a.show_record(ui, email)).unwrap();
        assert_eq!(
            app.g.selected_nodes(),
            &[node(&app, "email_contract_ready")]
        );
        assert!(app.g.selected_edges().is_empty());
        assert!(
            app.pending_camera.is_some(),
            "the board recentres on the record"
        );

        let director = event(&d, Lane::Event, "Vale — director of").key.record;
        let Record::Rel { edge, .. } = director else {
            panic!("a relation mark")
        };
        harness::frame(&mut app, |a, ui| a.show_record(ui, director)).unwrap();
        assert_eq!(app.g.selected_edges(), &[edge]);
        assert!(app.g.selected_nodes().is_empty());
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn a_board_selection_pins_its_earliest_mark_on_the_timeline() {
        let (_td, mut app) = demo_app();
        let company = node(&app, "brightwater_paving_srl");
        app.g.node_mut(company).unwrap().set_selected(true);
        app.g.set_selected_nodes(vec![company]);
        app.timeline.open = true;
        harness::frame(&mut app, |a, ui| a.timeline_panel(ui)).unwrap();

        let key = app
            .timeline
            .selected
            .expect("following the board pins a mark");
        let d = Digest::of(&app.g).unwrap();
        let pinned = d.events.iter().find(|e| e.key == key).unwrap();
        assert_eq!(pinned.key.lane, Lane::Event);
        assert_eq!(
            pinned.when.label().unwrap(),
            "2017",
            "the earliest account, even the disputed one"
        );
        assert_eq!(
            app.timeline.pending_fit, None,
            "the frame consumed the fit-to-selection"
        );
    }

    #[test]
    fn the_panel_renders_headlessly_with_and_without_a_pinned_mark() {
        let (_td, mut app) = demo_app();
        app.timeline.open = true;
        harness::frame(&mut app, |a, ui| a.timeline_panel(ui)).unwrap();
        assert!(
            app.timeline.selected.is_none(),
            "nothing is pinned until something is picked"
        );

        let d = Digest::of(&app.g).unwrap();
        app.timeline.selected = Some(event(&d, Lane::Event, "Tender 14/2019").key);
        app.timeline.pending_fit = Some(Fit::All);
        for lane in Lane::ALL {
            harness::frame(&mut app, |a, ui| a.timeline_panel(ui)).unwrap();
            app.timeline.lanes[lane.slot().unwrap()] = false;
        }
        harness::frame(&mut app, |a, ui| a.timeline_panel(ui)).unwrap();
    }

    #[test]
    fn the_demo_case_is_structurally_clean_and_round_trips() {
        let td = harness::temp_dir().unwrap();
        let project = fixtures::open_timeline_case(&td.path.join("case")).unwrap();
        let cf = crate::project::read_case(&project).unwrap();
        invariant::check_case_file(&cf).unwrap();
        let (_td2, other) = harness::temp_project().unwrap();
        let back = harness::round_trip(&other, &cf).unwrap();
        invariant::assert_round_trip(&cf, &back).unwrap();
        let published = cf
            .data
            .relations
            .iter()
            .find(|r| r.kind == RelationKind::DirectorOf)
            .unwrap()
            .sources[0]
            .published;
        assert_eq!(
            published,
            Some(DateVal {
                secs: 1_548_633_600,
                precision: DatePrecision::Day
            })
        );
    }
}
