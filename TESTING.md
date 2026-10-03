# Testing redstring

redstring is tested **entirely headlessly** — `cargo test` never launches eframe or a
window. This document explains the testkit substrate and how to add 100 more tests cheaply.

```sh
cargo test            # the whole suite (bin unit tests; no GUI)
cargo test ui::testkit   # just the scenario/table/round-trip/lookup wave
cargo test -- --ignored  # run the regressions gated on unlanded fixes
```

## Where things live

redstring is a **binary-only crate**, so all tests are in-crate `#[cfg(test)]` modules
(there is no `lib.rs`, so a `tests/` integration dir cannot reach the internals). The
shared test-support layer — the **testkit** — is a `#[cfg(test)]`-gated module tree:

- `src/ui/testkit/` — the main harness. It can construct a real `RedstringApp` and reach
  every UI-layer internal because it lives inside the `ui` module tree.
  - `harness.rs` — build a headless app, run a headless egui frame, temp dirs, save/load.
  - `graph.rs` — build/inspect the graph (add entities, link edges, set observations,
    read back nodes/edges/slugs/values/observations).
  - `invariant.rs` — the global invariant checker (call it at the end of every scenario).
  - `fixtures.rs` — load the checked-in tool-output fixtures and the sample case.
  - `apply.rs` — drive the lookup fact-application pipeline end to end.
  - `tests_*.rs` — the first wave of tests, one file per theme.
- `src/modules/testkit.rs` — a tiny `parse(spec_ron, raw) -> (shape_id, facts)` seam. It
  lives inside the `modules` tree because `extract::sniff` / `emit::emit` are private there.

The testkit compiles **only** under `cargo test`; it adds nothing to `cargo build` / the
shipped binary. The `Error` enum carries two `#[cfg(test)]`-gated variants for it:
`InvariantViolated { what }` and `Testkit { what }`.

## Writing a scenario test

A scenario builds some state through the **real app operations**, then asserts, then calls
the invariant checker so any future feature that breaks a hard rule fails loudly.

```rust
#[cfg(test)]                          // this exact line exempts the file from build.rs's
mod tests {                           // no-unwrap law — put test bodies below it
    use crate::model::{EntityKind, RelationKind};
    use crate::ui::testkit::{graph, harness, invariant};

    #[test]
    fn a_domain_owned_by_a_shell_is_consistent() {
        let mut app = harness::app().unwrap();               // headless app, no eframe
        let shell = graph::add(&mut app, EntityKind::Organization, "Shell Co Ltd").unwrap();
        let dom = graph::add_valued(&mut app, EntityKind::Domain, "evil.example", "evil.example").unwrap();
        graph::link(&mut app, shell, dom, RelationKind::OwnsDomain).unwrap();

        assert_eq!(graph::node_count(&app).unwrap(), 2);
        assert!(graph::has_edge(&app, shell, dom, RelationKind::OwnsDomain).unwrap());
        invariant::check_graph(&app).unwrap();               // ALWAYS end with this
    }
}
```

Test bodies use `.unwrap()` freely (build.rs exempts everything after a `#[cfg(test)]`
line). The **reusable testkit helpers stay clean** — every `pub fn` returns
`Result<_, Error>`, no `.unwrap()`, so tests use `?`-free `.unwrap()` on their return.

### Driving the UI layer headlessly

Anything that needs a live `egui::Ui` (save, load, the lookup pipeline) runs inside one
synthetic frame:

```rust
harness::frame(&mut app, |app, ui| app.poll_module_runs(ui))?;   // one headless frame
harness::save(&mut app).unwrap();                                // save to app.project
harness::load(&mut app).unwrap();                                // reload from disk
```

### Driving the autosave clock

The autosave debounce and the idle beat read **egui frame time**, so the clock is injectable
and no test ever sleeps. `harness::tick(&mut app, t)` runs one headless frame at time `t`
and calls `app.autosave_tick(ui)`; `harness::frame_at` does the same for any other body.

```rust
graph::add(&mut app, EntityKind::Person, "late arrival").unwrap();
harness::tick(&mut app, 0.0).unwrap();    // arms the debounce, writes nothing
harness::tick(&mut app, 1.6).unwrap();    // > DEBOUNCE_SECS — flushed to disk
harness::tick(&mut app, 13.0).unwrap();   // > IDLE_SECS — derived artifacts regenerate
```

Anything that must observe the **camera** has to write and read it inside one `egui::Context`
(each `harness::frame` builds a fresh context, so camera state does not survive between
calls) — drive `ctx.run_ui` directly and call `camera_write` then `app.autosave_tick` in the
same closure. See `tests_autosave.rs`.

### Round-trip identity

`read_case` returns entities in slug-sorted order, so raw `CaseData` equality is fragile;
use the ordering-aware assertion:

```rust
let back = harness::round_trip(&project, &cf).unwrap();  // write_case + read_case
invariant::assert_round_trip(&cf, &back).unwrap();       // identity up to canonical order
invariant::check_case_file(&back).unwrap();
```

## The invariant checker

`invariant::check_graph(&app)` (live graph) and `invariant::check_case_file(&cf)` /
`invariant::check_case_dir(&project)` (persisted) assert the model's hard rules and return
the **first** violation by name. They enforce:

1. **Unique slugs** — no two nodes share a slug.
2. **Slug tracks label** — every slug is the canonical `slugify(label)` (with an optional
   `_N` uniqueness suffix) or `slugify(value)`; a slug divorced from both is a violation.
3. **No duplicate entities** — no two nodes with a non-empty value share
   `(kind, normalized-value)`.
4. **Lookup edges are sourced** — every relation with `Origin::Lookup(_)` carries ≥1 source.
5. **Observations are stamped** — every observation has a non-empty id and author.
6. **Persisted extras** (`check_case_file`) — relations cite known slugs; `check_case_dir`
   also runs `--check`/`validate` and requires zero findings (no dangling cited paths).

Call one of these at the end of every scenario. The negative tests in
`tests_scenarios.rs` (`checker_rejects_*`) prove the checker actually bites.

## Table-driven coverage

The const model tables are the source of truth, so tests iterate them instead of
hand-listing kinds. One loop = dozens of assertions:

```rust
for m in KIND_META.iter() { /* build + save + load + check for each of the 62 kinds */ }
for m in RELATION_META.iter() { /* edge create + round-trip for each of the 147 verbs */ }
```

To cover a new kind/relation/source, add it to the const table — the table tests pick it
up automatically.

## Fixtures

Real (or clearly-synthetic) tool outputs live in `src/ui/testkit/fixtures/` and load via
`fixtures::fixture("name")` (compile-time `include_str!`):

| fixture | what it is |
|---|---|
| `whois-domain.txt` / `whois-ip.txt` | real-shaped whois responses (ICANN / ARIN) |
| `tlsx.jsonl` | tlsx leaf-cert record (post-`jq`, the ndjson the shape consumes) |
| `crtsh.jsonl` | crt.sh SANs, post-`jq` ndjson |
| `crtsh-response.json` | the raw crt.sh JSON array (pre-`jq`) |
| `crtsh-404.html` | a flaky HTML error body — proves graceful degrade to zero facts |
| `katana.jsonl` / `dnsx.jsonl` / `candel.jsonl` | crawl / DNS / username-sweep records |
| `alibi-check.jsonl` / `alibi-archive.jsonl` | synthetic `alibi --json` streams in the documented wire shape (check verdicts; archive skip → submitted → confirmed) — `tests_alibi.rs` replays them through fake `alibi` shell scripts, so nothing reaches the network |
| `sample_case/` | a full on-disk case dir (case.redstring + entities + relations + display) |
| `timeline_case/` | synthetic case for the timeline: every precision, zones, conflicts, unplaced records — `fixtures::open_timeline_case` |

Modules whose `bin` is `sh` (tlsx, crtsh, katana, dnsx, candel) pipe through `jq`, so their
ndjson fixtures are the **post-jq** lines the extractor actually sees.

### Adding a fixture

1. Drop the file in `src/ui/testkit/fixtures/`.
2. Add a `"name" => include_str!("fixtures/name")` arm to `fixtures::fixture`.
3. If you can't capture a real tool, hand-author a realistic body and note it's synthetic.

## The lookup pipeline, end to end

`apply::parse(spec_ron, raw)` runs `extract → sniff → emit` and returns `(shape_id, facts)`
with no subprocess. `apply::apply_to(app, spec_ron, id, target, raw)` runs the full
`extract → sniff → emit → apply` against the live graph (through `poll_module_runs`, the
app's real seam) and returns node/edge deltas. See `tests_lookups.rs`.

## Regressions gated on unlanded fixes

When a test encodes the **correct** behavior of a fix that hasn't landed on this branch yet,
gate it so the suite stays green and the test is ready to un-ignore at merge:

```rust
#[test]
#[ignore = "passes once <the fix> lands"]
fn asserts_the_post_fix_behavior() { /* … */ }
```

Run these explicitly with `cargo test -- --ignored`. Everything not ignored must be green.
**There are currently no ignored tests** — the whole suite runs by default.

## The export leak suite

`tests_export_leak.rs` is the one suite that must never be allowed to go vacuous. It builds a
case, runs a real lookup **that archives raw stdout to `evidence/`**, seals the entity, exports
the bundle, and then scans every entry NAME and every entry's BYTES for the real slug, the real
label, the real value and the secret inside the archived output.

The bug it exists to catch (`--export` shipping `evidence/docs/{module}-{real_slug}-{ts}.txt`
verbatim) hid for a long time precisely because every other seal test built a `CaseFile` in
memory and never ran a lookup, so `evidence/` was always empty and the redaction suite passed
while the leak was live. If you add a seal test, run a lookup first or you are testing nothing.
