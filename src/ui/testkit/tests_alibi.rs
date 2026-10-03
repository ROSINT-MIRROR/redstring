#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::channel;
    use std::time::{Duration, Instant};

    use petgraph::graph::EdgeIndex;

    use crate::alibi::config::{ArchiveKey, SearchEnv};
    use crate::alibi::exec::{AlibiEnd, AlibiEvent, AlibiRequest, alibi_args, run_alibi};
    use crate::alibi::locate::{Located, candidates, locate};
    use crate::alibi::wire::{Asked, parse_line, ts14_secs};
    use crate::model::{
        ArchiveRecord, ArchiveStatus, ArchiveVia, EntityKind, RelationKind, Source, SourceKind,
        merge_archive,
    };
    use crate::ui::RedstringApp;
    use crate::ui::archive::{Prepared, Tone};
    use crate::ui::testkit::{fixtures, graph, harness, invariant};

    const URL: &str = "https://evil.example/post";
    const OTHER: &str = "https://other.example/older";

    struct Fake {
        dir: harness::TempDir,
        bin: PathBuf,
    }

    impl Fake {
        fn argv(&self) -> String {
            std::fs::read_to_string(self.dir.path.join("argv")).unwrap()
        }

        fn key(&self) -> String {
            std::fs::read_to_string(self.dir.path.join("key")).unwrap()
        }

        fn ran(&self) -> bool {
            self.dir.path.join("argv").exists()
        }
    }

    fn fake(lines: &str, tail: &str) -> Fake {
        let dir = harness::temp_dir().unwrap();
        std::fs::write(dir.path.join("lines.jsonl"), lines).unwrap();
        let d = dir.path.display();
        let script = format!(
            "#!/bin/sh\n[ \"$1\" = --probe ] && exit 0\nprintf '%s\\n' \"$@\" > '{d}/argv'\nprintf '%s\\n' \"${{ARCHIVE_ORG_KEY-unset}}\" > '{d}/key'\ncat '{d}/lines.jsonl'\n{tail}\n"
        );
        let bin = dir.path.join("alibi");
        std::fs::write(&bin, script).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        for attempt in 0..200 {
            match std::process::Command::new(&bin).arg("--probe").status() {
                Ok(status) => {
                    assert!(status.success(), "probe failed: {status}");
                    break;
                }
                Err(err) if err.raw_os_error() == Some(26) && attempt < 199 => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(err) => panic!("fake alibi will not exec: {err}"),
            }
        }
        Fake { dir, bin }
    }

    fn check_lines() -> String {
        fixtures::fixture("alibi-check.jsonl").unwrap()
    }

    fn archive_lines() -> String {
        fixtures::fixture("alibi-archive.jsonl").unwrap()
    }

    fn request(bin: &Path, via: ArchiveVia, key: ArchiveKey, watchdog: Duration) -> AlibiRequest {
        AlibiRequest {
            job: 7,
            bin: bin.to_path_buf(),
            url: URL.to_owned(),
            via,
            key,
            watchdog,
            cancel: Default::default(),
        }
    }

    fn events_of(req: &AlibiRequest) -> (Result<(), crate::error::Error>, Vec<AlibiEvent>) {
        let (tx, rx) = channel();
        let outcome = run_alibi(req, &tx, &egui::Context::default());
        (outcome, rx.try_iter().collect())
    }

    fn readings(events: &[AlibiEvent]) -> Vec<&ArchiveRecord> {
        events
            .iter()
            .filter_map(|event| match event {
                AlibiEvent::Reading { record, .. } => Some(record),
                AlibiEvent::Done { .. } => None,
            })
            .collect()
    }

    fn end_of(events: &[AlibiEvent]) -> &AlibiEnd {
        match events.last().expect("a Done event closes every run") {
            AlibiEvent::Done { end, .. } => end,
            AlibiEvent::Reading { .. } => panic!("the last event must be Done"),
        }
    }

    fn case() -> (harness::TempDir, RedstringApp, EdgeIndex) {
        let (td, project) = harness::temp_project().unwrap();
        let mut app = harness::app().unwrap();
        app.project = Some(project);
        let a = graph::add(&mut app, EntityKind::Person, "Jane Roe").unwrap();
        let b = graph::add(&mut app, EntityKind::Organization, "Shell Co").unwrap();
        let src = graph::source(SourceKind::Url, OTHER, "").unwrap();
        let e = graph::link_sourced(&mut app, a, b, RelationKind::Controls, src).unwrap();
        let sources = &mut app.g.edge_mut(e).unwrap().payload_mut().sources;
        sources.push(graph::source(SourceKind::Url, &format!("  {URL} "), "the post").unwrap());
        sources.push(graph::source(SourceKind::Note, "call with the clerk", "").unwrap());
        (td, app, e)
    }

    fn the_edge(app: &RedstringApp) -> EdgeIndex {
        let a = graph::node_by_slug(app, "jane_roe").unwrap().unwrap();
        let b = graph::node_by_slug(app, "shell_co").unwrap().unwrap();
        app.g.g().find_edge(a, b).unwrap()
    }

    fn srcs(app: &RedstringApp) -> Vec<Source> {
        let a = graph::node_by_slug(app, "jane_roe").unwrap().unwrap();
        let b = graph::node_by_slug(app, "shell_co").unwrap().unwrap();
        graph::edge_sources(app, a, b, RelationKind::Controls).unwrap()
    }

    fn pin(app: &mut RedstringApp, fake: &Fake) {
        app.prefs.alibi_bin = Some(fake.bin.display().to_string());
    }

    fn ready(app: &mut RedstringApp, e: EdgeIndex, source: usize, via: ArchiveVia) -> AlibiRequest {
        match app.alibi_prepare(e, source, via).unwrap() {
            Prepared::Ready(req) => req,
            Prepared::Refused => panic!("refused: {}", app.status),
        }
    }

    fn refused(app: &mut RedstringApp, e: EdgeIndex, source: usize, via: ArchiveVia) -> bool {
        matches!(app.alibi_prepare(e, source, via).unwrap(), Prepared::Refused)
    }

    fn finish(app: &mut RedstringApp, req: &AlibiRequest) {
        let tx = app.alibi.tx.clone();
        run_alibi(req, &tx, &egui::Context::default()).unwrap();
        app.poll_alibi().unwrap();
    }

    fn last_note(app: &RedstringApp) -> (String, Tone) {
        let note = app.alibi.notes.last().expect("a note");
        (note.text.clone(), note.tone)
    }

    fn by_witness<'a>(recs: &'a [ArchiveRecord], witness: &str) -> Vec<&'a ArchiveRecord> {
        recs.iter().filter(|rec| rec.witness == witness).collect()
    }

    #[test]
    fn wire_reads_every_documented_status_verbatim() {
        let asked = Asked { url: URL, via: ArchiveVia::Check, now: 1_800_000_000 };
        let recs: Vec<ArchiveRecord> =
            check_lines().lines().map(|line| parse_line(line, &asked).unwrap()).collect();
        assert_eq!(recs[0].status, ArchiveStatus::Confirmed);
        assert_eq!(
            recs[0].snapshot.as_deref(),
            Some("https://web.archive.org/web/20260101000000/https://evil.example/post")
        );
        assert_eq!(recs[0].archived_at, Some(1_767_225_600), "ts14 is UTC, converted exactly");
        assert_eq!((recs[1].status, recs[1].reason.as_str()), (ArchiveStatus::Absent, "absent"));
        assert_eq!(recs[1].snapshot, None);
        assert_eq!(recs[2].status, ArchiveStatus::Confirmed);
        assert_eq!(recs[2].archived_at, None, "ghostarchive publishes no date — none is invented");
        for rec in &recs {
            assert_eq!((rec.url.as_str(), rec.via, rec.checked_at), (URL, ArchiveVia::Check, 1_800_000_000));
        }

        let asked = Asked { url: URL, via: ArchiveVia::Archive, now: 1_800_000_100 };
        let recs: Vec<ArchiveRecord> =
            archive_lines().lines().map(|line| parse_line(line, &asked).unwrap()).collect();
        assert_eq!((recs[0].status, recs[0].reason.as_str()), (ArchiveStatus::Skipped, "no_key"));
        assert!(recs[0].detail.as_deref().unwrap().contains("ARCHIVE_ORG_KEY"));
        assert_eq!(recs[1].status, ArchiveStatus::Submitted);
        assert_eq!(recs[3].archived_at, Some(ts14_secs("20261003120501").unwrap()));

        for (line, status, reason) in [
            (r#"{"witness":"ghostarchive","url":"https://evil.example/post","status":"pending","reason":"rate_limited","snapshot":null,"ts":null,"detail":"http 429"}"#, ArchiveStatus::Pending, "rate_limited"),
            (r#"{"witness":"archivetoday","url":"https://evil.example/post","status":"error","reason":"network","snapshot":null,"ts":null,"detail":"dns"}"#, ArchiveStatus::Errored, "network"),
        ] {
            let rec = parse_line(line, &asked).unwrap();
            assert_eq!((rec.status, rec.reason.as_str()), (status, reason));
        }

        let real = r#"{"witness":"wayback","url":"https://x.example/","status":"skipped","reason":"no_key","snapshot":null,"ts":null,"detail":"ARCHIVE_ORG_KEY not set — free key: https://archive.org/account/s3.php"}"#;
        let asked = Asked { url: "https://x.example/", via: ArchiveVia::Archive, now: 1 };
        assert_eq!(
            parse_line(real, &asked).unwrap().status,
            ArchiveStatus::Skipped,
            "a line captured from the real alibi 0.1.0 binary parses"
        );
    }

    #[test]
    fn wire_rejects_every_contract_break_instead_of_recording_it() {
        let asked = Asked { url: URL, via: ArchiveVia::Check, now: 1 };
        let line = |status: &str, snapshot: &str, ts: &str, url: &str, witness: &str| {
            format!(r#"{{"witness":"{witness}","url":"{url}","status":"{status}","reason":"ok","snapshot":{snapshot},"ts":{ts},"detail":null}}"#)
        };
        let snap = r#""https://web.archive.org/web/20260101000000/https://evil.example/post""#;
        for bad in [
            "wayback\thttps://web.archive.org/web/x".to_owned(),
            line("archived", snap, "null", URL, "wayback"),
            line("confirmed", "null", "null", URL, "wayback"),
            line("confirmed", r#""javascript:alert(1)""#, "null", URL, "wayback"),
            line("confirmed", snap, r#""20261332000000""#, URL, "wayback"),
            line("confirmed", snap, r#""2026010100000""#, URL, "wayback"),
            line("confirmed", snap, r#""20260230000000""#, URL, "wayback"),
            line("confirmed", snap, "null", "https://evil.example/other", "wayback"),
            line("confirmed", snap, "null", URL, "Way Back"),
        ] {
            let err = parse_line(&bad, &asked).expect_err(&bad).to_string();
            assert!(err.starts_with("alibi:"), "{err}");
        }
    }

    #[test]
    fn ts14_converts_known_utc_instants() {
        assert_eq!(ts14_secs("19700101000000").unwrap(), 0);
        assert_eq!(ts14_secs("20260101000000").unwrap(), 1_767_225_600);
        assert_eq!(ts14_secs("20000229235959").unwrap(), 951_868_799);
    }

    #[test]
    fn merge_keeps_every_snapshot_and_only_the_latest_verdict_per_witness() {
        let rec = |status, snapshot: Option<&str>, at: i64| ArchiveRecord {
            url: URL.to_owned(),
            witness: "wayback".to_owned(),
            status,
            reason: "ok".to_owned(),
            snapshot: snapshot.map(str::to_owned),
            archived_at: None,
            detail: None,
            via: ArchiveVia::Check,
            checked_at: at,
        };
        let mut kept = Vec::new();
        assert!(merge_archive(&mut kept, rec(ArchiveStatus::Absent, None, 1)).unwrap());
        assert!(merge_archive(&mut kept, rec(ArchiveStatus::Pending, None, 2)).unwrap());
        assert_eq!(kept.len(), 1, "a newer verdict replaces the older one");
        assert!(merge_archive(&mut kept, rec(ArchiveStatus::Confirmed, Some("https://w/1"), 3)).unwrap());
        assert_eq!(kept.len(), 1, "a confirmation supersedes the stale pending");
        assert!(!merge_archive(&mut kept, rec(ArchiveStatus::Confirmed, Some("https://w/1"), 4)).unwrap());
        assert_eq!(kept[0].checked_at, 3, "a re-seen snapshot keeps its first sighting");
        merge_archive(&mut kept, rec(ArchiveStatus::Confirmed, Some("https://w/2"), 5)).unwrap();
        merge_archive(&mut kept, rec(ArchiveStatus::Absent, None, 6)).unwrap();
        let statuses: Vec<ArchiveStatus> = kept.iter().map(|k| k.status).collect();
        assert_eq!(
            statuses,
            vec![ArchiveStatus::Confirmed, ArchiveStatus::Confirmed, ArchiveStatus::Absent],
            "a takedown shows as the latest verdict and never erases a confirmed snapshot"
        );
        let mut elsewhere = rec(ArchiveStatus::Absent, None, 7);
        elsewhere.url = OTHER.to_owned();
        merge_archive(&mut kept, elsewhere).unwrap();
        assert_eq!(kept.len(), 4, "verdicts are per URL");
    }

    #[test]
    fn args_pin_the_documented_cli_contract() {
        assert_eq!(alibi_args(ArchiveVia::Check, URL).unwrap(), vec!["--check", "--json", URL]);
        assert_eq!(
            alibi_args(ArchiveVia::Archive, URL).unwrap(),
            vec!["--json", "--timeout", "300", URL]
        );
    }

    #[test]
    fn locate_walks_pinned_then_path_then_cargo_and_home_bins() {
        let td = harness::temp_dir().unwrap();
        let root = &td.path;
        let env = SearchEnv {
            pinned: None,
            path: vec![root.join("p1"), PathBuf::from("relative"), root.join("p2")],
            cargo_home: Some(root.join("cargo")),
            home: Some(root.join("home")),
        };
        let want: Vec<PathBuf> = vec![
            root.join("p1/alibi"),
            root.join("p2/alibi"),
            root.join("cargo/bin/alibi"),
            root.join("home/.cargo/bin/alibi"),
            root.join("home/.local/share/cargo/bin/alibi"),
            root.join("home/.local/bin/alibi"),
        ];
        assert_eq!(candidates(&env).unwrap(), want, "relative PATH entries are never trusted");
        let Located::Missing(tried) = locate(&env).unwrap() else { panic!("nothing installed yet") };
        assert_eq!(tried, want, "the miss reports every place it looked");

        for dir in ["p2", "home/.local/bin"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("p2/alibi"), "not executable").unwrap();
        let installed = root.join("home/.local/bin/alibi");
        std::fs::write(&installed, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&installed, std::fs::Permissions::from_mode(0o755)).unwrap();
        let Located::Found(bin) = locate(&env).unwrap() else { panic!("installed in ~/.local/bin") };
        assert_eq!(bin, installed, "a non-executable file earlier in PATH is skipped");

        let pinned = SearchEnv { pinned: Some(root.join("nowhere/alibi")), ..env };
        let Located::Missing(tried) = locate(&pinned).unwrap() else {
            panic!("a broken pin must not fall back to PATH")
        };
        assert_eq!(tried, vec![root.join("nowhere/alibi")]);
    }

    #[test]
    fn run_streams_every_line_then_reports_the_exit() {
        let fake = fake(&check_lines(), "exit 2");
        let req = request(&fake.bin, ArchiveVia::Check, ArchiveKey::FromPrefs("ak:sk".to_owned()), Duration::from_secs(20));
        let (outcome, events) = events_of(&req);
        outcome.unwrap();
        assert_eq!(readings(&events).len(), 3);
        assert!(matches!(end_of(&events), AlibiEnd::Exited { code: 2, .. }));
        assert_eq!(fake.argv(), format!("--check\n--json\n{URL}\n"));
        assert_eq!(fake.key(), "unset\n", "a read-only check never carries the archive key");
    }

    #[test]
    fn archive_passes_the_prefs_key_through_env_never_argv() {
        let fake = fake(&archive_lines(), "exit 1");
        let req = request(&fake.bin, ArchiveVia::Archive, ArchiveKey::FromPrefs("ak:sk".to_owned()), Duration::from_secs(20));
        let (outcome, events) = events_of(&req);
        outcome.unwrap();
        assert_eq!(fake.key(), "ak:sk\n");
        assert_eq!(fake.argv(), format!("--json\n--timeout\n300\n{URL}\n"));
        assert!(!fake.argv().contains("ak:sk"));
        assert_eq!(readings(&events).len(), 4);
    }

    #[test]
    fn cancel_kills_promptly_and_keeps_what_alibi_already_said() {
        let fake = fake(&archive_lines(), "sleep 30");
        let req = request(&fake.bin, ArchiveVia::Archive, ArchiveKey::Inherited, Duration::from_secs(25));
        let flag = req.cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            flag.store(true, Ordering::Relaxed);
        });
        let t0 = Instant::now();
        let (outcome, events) = events_of(&req);
        outcome.unwrap();
        assert!(t0.elapsed() < Duration::from_secs(10), "cancel beat the 30s sleep");
        assert!(matches!(end_of(&events), AlibiEnd::Cancelled { .. }));
        assert_eq!(readings(&events).len(), 4, "lines printed before the kill are kept");
    }

    #[test]
    fn watchdog_kills_a_hung_alibi() {
        let fake = fake("", "sleep 30");
        let req = request(&fake.bin, ArchiveVia::Check, ArchiveKey::Inherited, Duration::from_secs(1));
        let t0 = Instant::now();
        let (outcome, events) = events_of(&req);
        outcome.unwrap();
        assert!(t0.elapsed() < Duration::from_secs(10));
        assert!(matches!(end_of(&events), AlibiEnd::TimedOut { .. }));
    }

    #[test]
    fn a_contract_break_stops_the_job_as_an_error_and_kills_alibi() {
        let fake = fake("wayback\thttps://web.archive.org/web/x\n", "sleep 30");
        let req = request(&fake.bin, ArchiveVia::Check, ArchiveKey::Inherited, Duration::from_secs(25));
        let t0 = Instant::now();
        let (outcome, events) = events_of(&req);
        let err = outcome.expect_err("tab output is not the --json contract").to_string();
        assert!(err.contains("--json contract"), "{err}");
        assert!(t0.elapsed() < Duration::from_secs(10), "the hung child was killed, not awaited");
        assert!(readings(&events).is_empty());
    }

    #[test]
    fn a_missing_binary_is_a_launch_error() {
        let req = request(Path::new("/nonexistent/sherlockit/alibi"), ArchiveVia::Check, ArchiveKey::Inherited, Duration::from_secs(5));
        let (outcome, events) = events_of(&req);
        assert!(outcome.expect_err("no binary").to_string().contains("cannot launch"));
        assert!(events.is_empty());
    }

    #[test]
    fn check_attaches_to_exactly_that_source_and_survives_save_and_reload() {
        let (_td, mut app, e) = case();
        let fake = fake(&check_lines(), "exit 2");
        pin(&mut app, &fake);
        let req = ready(&mut app, e, 1, ArchiveVia::Check);
        finish(&mut app, &req);

        assert!(app.alibi.jobs.is_empty(), "the job closed on Done");
        let sources = srcs(&app);
        assert_eq!(sources[1].archives.len(), 3);
        assert_eq!(sources[1].reference, format!("  {URL} "), "the cited URL is never rewritten");
        assert_eq!((sources[1].fetched_at, sources[1].published), (None, None), "no retrieval or publication time is invented");
        assert!(sources[0].archives.is_empty() && sources[2].archives.is_empty(), "sibling sources untouched");
        let (text, tone) = last_note(&app);
        assert_eq!(tone, Tone::Done);
        assert!(text.contains("2 confirmed, 1 without a snapshot"), "{text}");

        harness::save(&mut app).unwrap();
        harness::load(&mut app).unwrap();
        assert_eq!(srcs(&app)[1].archives, sources[1].archives, "records persist in relations.ndjson");
        invariant::check_graph(&app).unwrap();

        let e = the_edge(&app);
        let again = ready(&mut app, e, 1, ArchiveVia::Check);
        finish(&mut app, &again);
        assert_eq!(srcs(&app)[1].archives.len(), 3, "a re-check dedupes snapshots and replaces verdicts");
    }

    #[test]
    fn archive_records_unresolved_submissions_and_never_invents_a_confirmation() {
        let (_td, mut app, e) = case();
        let fake = fake(&archive_lines(), "sleep 30");
        pin(&mut app, &fake);
        let req = ready(&mut app, e, 1, ArchiveVia::Archive);
        let flag = req.cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            flag.store(true, Ordering::Relaxed);
        });
        finish(&mut app, &req);

        let recs = srcs(&app)[1].archives.clone();
        assert_eq!(by_witness(&recs, "wayback")[0].status, ArchiveStatus::Skipped);
        let today = by_witness(&recs, "archivetoday");
        assert_eq!((today.len(), today[0].status), (1, ArchiveStatus::Confirmed), "its submitted line was resolved by the confirmation");
        assert_eq!(today[0].snapshot.as_deref(), Some("https://archive.ph/20261003120501/https://evil.example/post"));
        assert_eq!(today[0].via, ArchiveVia::Archive);
        let ghost = by_witness(&recs, "ghostarchive");
        assert_eq!((ghost.len(), ghost[0].status), (1, ArchiveStatus::Submitted), "submitted, never promoted to confirmed");
        assert!(ghost[0].detail.as_deref().unwrap().contains("429"));
        let (text, tone) = last_note(&app);
        assert_eq!(tone, Tone::Failed);
        assert!(text.contains("cancelled") && text.contains("1 submitted with no verdict yet"), "{text}");
    }

    #[test]
    fn duplicate_and_concurrent_submissions_are_refused() {
        let (_td, mut app, e) = case();
        let fake = fake(&check_lines(), "exit 2");
        pin(&mut app, &fake);
        ready(&mut app, e, 1, ArchiveVia::Check);
        assert!(refused(&mut app, e, 1, ArchiveVia::Check), "same URL already in flight");
        assert!(refused(&mut app, e, 1, ArchiveVia::Archive), "no archive while a check of it runs");
        assert!(last_note(&app).0.contains("still running"));
        ready(&mut app, e, 0, ArchiveVia::Archive);
        app.alibi.jobs.retain(|job| job.via == ArchiveVia::Archive);
        assert!(refused(&mut app, e, 1, ArchiveVia::Archive), "one archive submission at a time");
        ready(&mut app, e, 1, ArchiveVia::Check);
        assert_eq!(app.alibi.jobs.len(), 2);
        assert!(!fake.ran(), "preparing spawns nothing");
    }

    #[test]
    fn refusals_cover_non_url_sealed_and_missing_tool() {
        let (_td, mut app, e) = case();
        let fake = fake(&check_lines(), "exit 2");
        pin(&mut app, &fake);
        assert!(refused(&mut app, e, 2, ArchiveVia::Check), "a Note source is not a URL");

        app.g.edge_mut(e).unwrap().payload_mut().sealed = true;
        assert!(refused(&mut app, e, 1, ArchiveVia::Archive));
        assert!(last_note(&app).0.contains("sealed"));
        ready(&mut app, e, 1, ArchiveVia::Check);
        app.alibi.jobs.clear();
        app.g.edge_mut(e).unwrap().payload_mut().sealed = false;

        app.prefs.alibi_bin = Some("/nonexistent/sherlockit/alibi".to_owned());
        assert!(refused(&mut app, e, 1, ArchiveVia::Check));
        assert!(app.alibi.jobs.is_empty());
        let (text, tone) = last_note(&app);
        assert_eq!(tone, Tone::Failed);
        assert!(text.contains("not installed") && text.contains("/nonexistent/sherlockit/alibi"), "{text}");
        assert!(app.status.contains("not installed"));
    }

    #[test]
    fn results_never_land_on_a_source_that_changed_mid_job() {
        let (_td, mut app, e) = case();
        let stub = fake(&check_lines(), "exit 2");
        pin(&mut app, &stub);

        let req = ready(&mut app, e, 1, ArchiveVia::Check);
        app.g.edge_mut(e).unwrap().payload_mut().sources[1].reference = "https://evil.example/edited".to_owned();
        finish(&mut app, &req);
        assert!(srcs(&app).iter().all(|s| s.archives.is_empty()), "an edited URL gets nothing");
        assert!(last_note(&app).0.contains("not attached"));

        app.g.edge_mut(e).unwrap().payload_mut().sources[1].reference = URL.to_owned();
        let a = graph::node_by_slug(&app, "jane_roe").unwrap().unwrap();
        let c = graph::add(&mut app, EntityKind::Organization, "Bystander").unwrap();
        graph::link_sourced(&mut app, a, c, RelationKind::Controls, graph::source(SourceKind::Url, URL, "").unwrap()).unwrap();
        let req = ready(&mut app, e, 1, ArchiveVia::Check);
        let saved = app.project.take();
        finish(&mut app, &req);
        assert!(srcs(&app).iter().all(|s| s.archives.is_empty()), "another case is open — dropped");
        app.project = saved;

        let req = ready(&mut app, e, 1, ArchiveVia::Check);
        finish(&mut app, &req);
        assert_eq!(srcs(&app)[1].archives.len(), 3);
        let other = graph::edge_sources(&app, a, c, RelationKind::Controls).unwrap();
        assert!(other[0].archives.is_empty(), "the same URL on another relation is a different citation");

        let req = ready(&mut app, e, 0, ArchiveVia::Check);
        app.g.remove_edge(e);
        let tx = app.alibi.tx.clone();
        let lines = fake(&check_lines().replace(URL, OTHER), "exit 2");
        let req = AlibiRequest { bin: lines.bin.clone(), ..req };
        run_alibi(&req, &tx, &egui::Context::default()).unwrap();
        app.poll_alibi().unwrap();
        assert!(last_note(&app).0.contains("3 result(s) not attached"));
        invariant::check_graph(&app).unwrap();
    }

    #[test]
    fn usage_errors_are_reported_and_never_recorded() {
        let (_td, mut app, e) = case();
        let fake = fake("", "echo 'alibi: url must start with http:// or https://' >&2\necho 'usage: …' >&2\nexit 64");
        pin(&mut app, &fake);
        let req = ready(&mut app, e, 1, ArchiveVia::Check);
        finish(&mut app, &req);
        let (text, tone) = last_note(&app);
        assert_eq!(tone, Tone::Failed);
        assert_eq!(text, "alibi exited 64: alibi: url must start with http:// or https://");
        assert!(srcs(&app)[1].archives.is_empty());
    }

    #[test]
    fn selecting_and_rendering_a_sourced_edge_never_runs_alibi() {
        let (_td, mut app, e) = case();
        let fake = fake(&check_lines(), "exit 2");
        pin(&mut app, &fake);
        app.g.edge_mut(e).unwrap().set_selected(true);
        for _frame in 0..3 {
            harness::frame(&mut app, |a, ui| a.inspector(ui)).unwrap();
            app.poll_alibi().unwrap();
        }
        assert!(app.alibi.jobs.is_empty());
        assert!(!fake.ran(), "selection and redraw spawn nothing");

        let req = ready(&mut app, e, 1, ArchiveVia::Check);
        finish(&mut app, &req);
        harness::frame(&mut app, |a, ui| a.inspector(ui)).unwrap();
        assert!(app.alibi.jobs.is_empty(), "drawing the archive rows schedules nothing");
    }

    #[test]
    fn the_threaded_path_attaches_results_and_surfaces_worker_errors() {
        let (_td, mut app, e) = case();
        let ctx = egui::Context::default();
        let good = fake(&check_lines(), "exit 0");
        pin(&mut app, &good);
        app.alibi_start(&ctx, e, 1, ArchiveVia::Check).unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !app.alibi.jobs.is_empty() && Instant::now() < deadline {
            app.poll_alibi().unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(srcs(&app)[1].archives.len(), 3);

        let broken = fake("not json\n", "sleep 30");
        pin(&mut app, &broken);
        app.alibi_start(&ctx, e, 1, ArchiveVia::Check).unwrap();
        let mut surfaced = None;
        while surfaced.is_none() && Instant::now() < deadline {
            if let Err(err) = app.poll_alibi() {
                surfaced = Some(err.to_string());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let err = surfaced.expect("the worker error reaches the frame");
        assert!(err.contains("--json contract"), "{err}");
        assert!(app.alibi.jobs.is_empty());
        assert!(last_note(&app).0.starts_with("stopped: alibi: output broke"));
        assert_eq!(srcs(&app)[1].archives.len(), 3, "earlier records survive a later broken run");
    }
}
