# redstring

Graph-based investigation tool for journalism. Rust, local-first, no server.

The metaphor is the detective corkboard: entities are the pinned photos, edges
are the red string — and **every string carries the source that justifies it.**
A connection you can't source is a connection you can't publish.

> ## Status: in development
>
> It builds, the test suite is green, the core loop works end to end, and the
> four security blockers found by auditing this tree are now **fixed** — command
> injection from hostile lookup data, the bundle export that leaked `sealed`
> entities, brute-forceable sealed aliases, and the dead OpenTimestamps fields.
> Each fix carries a regression test that fails when the fix is reverted.
>
> It has still never been run on a live investigation and has had no external
> review.
>
> Nothing here is released. No packages, no binaries, no crates.io entry.

## Why

- **Maltego** — bloated Java, paywalled transforms, edges are dumb lines with no
  provenance.
- **Obsidian** — proprietary, note-centric rather than relation-centric, the
  graph view is a toy.

`redstring` is the relational and evidentiary overlay on an investigation. It
doesn't replace your notes, archives or transcripts — it cites *into* them.

## Model

Four objects. The edge is where the novelty is; the observation is where the
rigor is.

- **Entity** — a person, org, domain, address, wallet. 62 kinds.
- **Edge** — typed, directed, and carrying one or more **Sources**, stamped with
  origin, status, confidence, author and time. 144 verbs. An edge with zero
  sources is an unproven assertion and is drawn as one.
- **Observation** — a typed, individually sourceable fact on an entity, with its
  own origin, verification status, confidence and author. Not a flat key-value
  attribute.
- **Source** — the citation: kind, reference, excerpt, `sha256` of the raw
  captured bytes, fetched-at, credibility tier.

Sources attach to **edges and observations**, not just to nodes. That is the part
that makes output defensible.

### Standing — when a string turns red

Every relation carries an analyst-set status, picked in the relation inspector
under **standing**: `unreviewed`, `machine`, `asserted`, `confirmed · primary`,
`corroborated`, `rejected`, `denied`, `no record`. A status is a claim about the
evidence, so it is checked against the sources actually attached:

- **confirmed · primary** holds only with at least one source rated `primary`.
  One authoritative record can settle a narrow claim (an officer listed on the
  organisation's own page), and it is never presented as corroboration.
- **corroborated** holds only with 2+ independent origins — distinct
  `source_org`, else distinct reference. Two pages of one organisation are one origin.
- Rating a source `primary` never verifies anything by itself; an unsourced or
  inferred record never holds either status.

Only a status that holds draws the strong red yarn on the board, the `==>` arrow
in `graph.md`, a filled mark on the timeline and "confirmed" / "corroborated" in
the dossiers — all from one rule (`model::Footing`). The inspector greys out a
status the evidence cannot carry and says what it needs; `--check` flags any
record whose status outruns its sources as `OVERCLAIMED`. Exports recompute the
standing from the sources they actually publish, so a claim confirmed only by a
sealed source reads as a sourced claim in the public bundle.

### Sealed subjects

Any entity, observation, source or relation can be flagged `sealed`. The working
case keeps everything; the derived public artifacts (`graph.md`, `case.json`, the
per-entity dossiers and the exported bundle) redact it. A sealed entity is
scrubbed and slug-aliased, and **every edge touching it is dropped**, so the
remaining graph shape can't re-triangulate the identity.

The exported bundle is built by **allowlist** from the redacted case, not by
copying the case directory and rewriting parts of it: nothing ships unless the
redacted case cited it by name, and a final byte-level scan aborts the export if
a sealed slug, label or value appears in any path or any file. Sealed aliases are
random per export, so two bundles of the same case cannot be correlated.

## Storage

A case is a directory of plain text, designed to diff and merge:

```
case/
├── case.redstring              # case metadata (RON)
├── display.ron                 # camera, positions, layout policy
├── relations.ndjson            # one edge per line
└── entities/<slug>/
    ├── entity.ron
    ├── observations.ndjson     # one fact per line
    └── note.md
```

One record per line, so two investigators on the same case merge in git like
code rather than fighting over a blob. Derived artifacts (`case.json`,
`graph.md`, `dossier.md`) are regenerated on save and are the publishable
surface.

## Build

Requires a recent stable Rust toolchain.

```sh
cargo build --release
cargo test              # 626 tests, fully headless — never opens a window
```

If your global `~/.cargo/config.toml` sets a non-LLVM codegen backend, override
it locally: the wgpu/egui stack uses SIMD intrinsics cranelift can't codegen yet.
A `.cargo/config.toml` in this directory wins the hierarchy and is gitignored,
because `codegen-backend` is nightly-only and committing it breaks every clone on
stable.

## Use

```sh
redstring                       # open the GUI
redstring --check [case]        # validate + source audit; exits 2 / 1 / 0
redstring --derive [case]       # regenerate case.json, graph.md, dossiers
redstring --export [case]       # public bundle, sealed material redacted
redstring --shot [case] --out board.png    # headless render, no window
```

`--check` is the one to wire into CI. It reports structural errors, then flags
every unproven (0-source) and unreviewed single-source edge, every status that
outruns its sources, plus uncited observations:

```
warn: UNPROVEN edge shell_co_ltd --owns domain--> evil-corp.example: 0 sources
warn: single-source edge j_doe --controls--> shell_co_ltd: 1 source — needs corroboration
summary: 0 structural error(s), 5 advisory warning(s) — 1 unproven (0-src) + 1 single-source edge(s), 0 overclaimed status, 3 uncited obs
```

Exit codes: `2` structural errors, `1` advisory warnings only, `0` clean.

## Timeline

`Ctrl+T` (or **Timeline** in the toolbar) docks a timeline under the board. It is
a live projection of the same case — nothing is copied or stored separately —
laid out on four lanes so the times of one record are never confused:

- **event** — when it happened: date observations, date-range observations and
  relation `valid_from`/`valid_to` windows, each at its stated precision. A month
  or a year is drawn as a band over the whole period, never as a point.
- **published** — when the cited material was made public (`Source.published`,
  editable on every source; never derived from anything else).
- **captured** — when the evidence was fetched or received (`Source.fetched_at`).
- **entered** — when the record entered the case (`created_at`; a record without
  a stamp is left off and counted, not given one).

Times of day are shown only when a record states one, read verbatim from an
ISO-8601 text value: `Z` and `±hh:mm` offsets are kept as written; a time without
a zone is placed by its day and labelled as such. Nothing else is parsed or guessed.

Marks are drawn by footing: corroborated or confirmed from a primary source
(filled, in the status colour), sourced claim (tinted),
unproven — no source or `Inference` — (dashed, `?`), disputed (`!`), rejected
(struck, `×`). Two accounts of the same fact whose dates cannot both hold are both
kept and linked with `≠`. Records with no date anywhere are listed as unplaced.

Clicking a mark selects its entity or relation on the board and pins a card with
its sources (URLs and case files open from there), its standing, conflicting
accounts and the same record's other times; a red string joins them across the
lanes. Selecting on the board pins and frames that record's marks.

A synthetic demo case lives in `src/ui/testkit/fixtures/timeline_case/` —
fictional people, a reserved `.example` domain, invented documents. Copy it
somewhere disposable and run `redstring` inside the copy.

## Lookups

31 module specs in `modules/`, each a declarative RON file: which entity kinds it
applies to, what to run, how to extract facts, what nodes and edges to emit.
Machine-sourced results are content-hashed (`sha256` of the raw stdout) and the
hash is stored on the source.

Modules shell out to tools **you** install — `whois`, `subfinder`, `dnsx`,
`exiftool`, `curl`, `jq` and so on. Nothing is bundled, and the binary links no
HTTP client of its own: if a tool isn't on `PATH`, its module is inert.

Two of them look similar and are not. **`wayback`** CDX-sweeps a domain for the
historical URLs it once served — a *discovery* move, widening the graph.
**`alibi`** asks whether one specific URL is on the public record right now,
across three independent archives — a *verification* move, hardening a citation
you already have. Keep both.

`alibi` is the [sherlockit](../) sibling binary and runs `--check` only:
read-only, submits nothing, safe to fire from a board click. Archiving takes
minutes and is rate-limited per IP, so you run `alibi <url>` from the shell when
you mean it, then re-run the module to pick the snapshots up. Its ndjson shape
streams, so confirmed rows land on the board even when the process exits non-zero
because a different archive 503'd.

Placeholders in a module spec are bound as positional shell parameters, never
interpolated into shell syntax, so a hostile value from a lookup response cannot
become a command.

API keys live in `$XDG_CONFIG_HOME/redstring/prefs.ron`, never in this repo, and
are passed to modules as environment variables — or over stdin for the ones that
need an HTTP header, so a key never lands in `/proc/*/cmdline`.

## Tests

```sh
cargo test                      # 626 tests, ~1s
cargo test ui::testkit          # scenario / round-trip / lookup suites
```

Entirely headless — no eframe, no window, no sleeping. See
[TESTING.md](TESTING.md) for the testkit and how to add cases. `build.rs` also
enforces a per-file LOC ceiling and a no-silent-error rule over non-test code.

## License

[0BSD](LICENSE). Public-domain-equivalent: use it, change it, ship it, no
attribution required.
