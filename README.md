# Red String

`redstring` is a local-first desktop tool for building investigative graphs —
people, organisations, places, documents, accounts and events, connected by
typed relations — where **every connection carries the source that justifies
it**. Written in Rust, it has no server or account, and stores each case as a
directory of plain-text files.

The metaphor is the detective's corkboard: entities are the pinned photos,
relations are the red string. The difference is that a string without a source
is drawn as what it is — an unproven assertion — and a claim is only shown as
confirmed when the evidence attached to it can carry that weight.

![The board: a synthetic demo case with sourced and unsourced relations](docs/screenshots/board.png)

> **Status: development preview, version 0.0.1.**
> It builds, the test suite passes, and the core loop (build a case, source it,
> check it, export it) works end to end. It has been used only by its author,
> has had no external review, and has only been run on Linux. There are no
> releases or packaged binaries; the case format may still change. See
> [Limitations](#limitations) before relying on it.

## The idea

Most link-analysis tools treat an edge as a line. In an investigation, the line
is the claim, and the claim is only as good as its evidence. Red String is built
around keeping those two things together, and around keeping a hypothesis
visibly distinct from an established fact:

- **Sources live on relations and on individual facts**, not only on nodes.
  Each source records what it is, where it is, an excerpt, when it was published,
  when it was captured, and a `sha256` of the captured bytes where there are any.
- **Every record carries its provenance**: origin (entered by hand, produced by a
  lookup, imported, or inferred), an analyst-set status, confidence, author and
  time of entry.
- **Unsourced is visible.** A relation with zero sources is drawn as a dashed
  string with a `0 src` warning; an inferred record is marked as such. Nothing is
  silently promoted to fact.
- **Theories have a sandbox.** *Explore* mode is a RAM-only copy of the case for
  trying out nodes, edges and hypotheses; nothing is written to disk unless you
  choose *Go live*.

The tool does not replace notes, archives or transcripts. It is the relational
and evidentiary layer that cites into them.

## Data model

- **Entity** — a person, organisation, place, domain, account, document, event,
  and so on: 62 kinds, grouped in the palette. An entity can show a portrait or
  logo image, or a country flag.
- **Relation** — typed and directed, chosen from 147 verbs in 22 families
  (ownership and control, roles, money, legal/state, movement, information,
  evidence, …). It carries one or more sources and its own origin, status,
  confidence, author, creation time and optional validity window.
- **Observation** — a typed fact on an entity (a date, an address, an
  identifier, …) with its own sources, origin, status, confidence and author.
  Facts are individually sourceable records, not a flat attribute list.
- **Source** — the citation: kind, reference (URL, file, note), excerpt,
  publication date, capture time, content hash, credibility tier, originating
  organisation, and any archive snapshots found for it.

### Standing: when a string turns red

Each relation has a status, picked in the inspector: `unreviewed`, `machine`,
`asserted`, `confirmed · primary`, `corroborated`, `rejected`, `denied`,
`no record`. A status is a claim about the evidence, so it is checked against
the sources actually attached:

- **confirmed · primary** holds only with at least one source rated `primary`;
  it is never presented as corroboration.
- **corroborated** holds only with two or more independent origins (distinct
  originating organisation, otherwise distinct reference).
- An unsourced or inferred record never holds either status.

Only a status that holds draws the strong red string on the board, the `==>`
arrow in `graph.md`, a filled mark on the timeline and "confirmed" /
"corroborated" in the dossiers. The inspector greys out a status the evidence
cannot carry, and `redstring --check` reports any record whose status outruns
its sources as `OVERCLAIMED`.

### Sealed subjects

Any entity, observation, source or relation can be marked **sealed**. The
working case keeps everything; the derived public files (`case.json`,
`graph.md`, dossiers) and the exported bundle redact it. A sealed entity is
replaced by a random per-export alias and every relation touching it is dropped,
so the remaining graph cannot be used to re-identify it. The export is built by
allowlist from the redacted case, and a final byte-level scan aborts it if any
sealed name or value would ship.

## What works today

**Board**
- Add entities from the kind palette; link two nodes by right-dragging from one
  to the other and picking a verb; edit everything in the inspector.
- Layouts: Free (hand-placed), Grid, Circle, Tree, Force and Radial, plus
  Loosen / Tighten spacing and Center. The chosen layout is kept per case.
- Relation labels on demand (*Labels*), node sizing by degree, folding of
  neighbourhoods, find (`Ctrl+F`), portrait/logo images that keep their
  transparency, and 249 bundled country flags (opt-in per entity, never guessed
  from a label).

**Timeline** (`Ctrl+T`) — the same case laid out on four separate lanes, so the
different times of one record are never confused: when it *happened*, when the
material was *published*, when it was *captured*, and when it was *entered* into
the case. Imprecise dates (a month, a year) are drawn as periods, conflicting
accounts are kept side by side and linked, and undated records are counted
rather than placed.

![Timeline docked under the board, with the inspector showing a record's sourced facts](docs/screenshots/timeline.png)

**Checks and outputs**
- `--check` validates the case and audits its sourcing (unproven relations,
  single-source relations, overclaimed statuses, uncited facts), with exit codes
  suitable for scripts and CI.
- `case.json`, `graph.md` and a per-entity `dossier.md` are regenerated from the
  case, with sealed material redacted.
- `--export` writes a redacted, shareable bundle (`<case>.redstring.zip`) that
  can be opened again in Red String.
- `--shot` renders the board headlessly to a PNG (or its geometry to JSON).

**Lookups** — 31 declarative module specs in `modules/` (WHOIS, DNS, certificate
transparency, crawlers, archives, …) that shell out to tools *you* install and
turn their output into sourced nodes and relations. Machine-sourced results are
content-hashed. If a tool is not on `PATH`, its module is simply inactive.
Values from lookups are passed as positional shell parameters, never
interpolated into shell syntax.

**Archive checks** — for any URL source, *Check existing archives* asks the
separate `alibi` command-line tool whether the page is on the public record
(read-only), and *Archive now* submits it to public web archives and records
each archive's answer on the source. Archiving sends the URL to third
parties and exposes your IP to them; sealed material is never submitted.

## Saving and working with others

A case is a directory of plain text, one record per line, so it diffs and merges
in git like code:

```
case/
├── case.redstring              # case metadata
├── display.ron                 # positions, camera (zoom and centre), layout
├── relations.ndjson            # one relation per line
├── entities/<slug>/
│   ├── entity.ron
│   ├── observations.ndjson     # one fact per line
│   ├── note.md
│   └── dossier.md              # derived
├── case.json                   # derived, redacted
└── graph.md                    # derived, redacted
```

- **Live mode** (the default) autosaves shortly after each edit settles. Zoom
  and camera centre are saved per case and restored when you reopen it.
  `Ctrl+S` saves now, `Ctrl+Shift+S` saves a copy elsewhere, `Ctrl+O` opens a
  case (you can also drop a case folder or a `.redstring.zip` on the window).
- **Explore mode** keeps every change in memory until you *Go live* or
  *Discard*.
- **Writes are atomic per file** and serialised with a lock file
  (`.redstring.lock`), so a crash never leaves a half-written file.
- **Edits made on disk while the case is open** — a `git pull`, a script, another
  tool — are merged into the open case three ways against the last synced
  state. Non-overlapping changes are absorbed (new nodes animate in without
  disturbing the layout, and the view re-frames the graph); if both sides changed
  the same record, a conflict window lets you keep yours or take theirs.

Preferences (theme, toolbar toggles, API keys for lookup modules) live in
`$XDG_CONFIG_HOME/redstring/prefs.ron`, never in a case or in this repository.
Keys are handed to modules through environment variables or stdin, not argv.

## Install and run

**Requirements**
- A recent stable Rust toolchain (edition 2024; developed on rustc 1.98).
- A GPU and driver supported by [wgpu](https://wgpu.rs) (Vulkan, Metal, DX12 or
  OpenGL) and a desktop session. Only Linux (X11 and Wayland) has been tested.
- Optional: the command-line tools used by the lookup modules (`whois`,
  `subfinder`, `dnsx`, `exiftool`, `curl`, `jq`, …) and `alibi` for archive
  checks.

**Build and install from source**

```sh
git clone https://github.com/ROSINT-MIRROR/redstring
cd redstring
cargo build --release                       # binary at target/release/redstring
cargo install --path . --locked             # or install it into ~/.cargo/bin
```

If your global `~/.cargo/config.toml` selects the Cranelift backend, override it
for this crate: the wgpu/egui stack uses SIMD intrinsics Cranelift cannot
compile yet. A local `.cargo/config.toml` is gitignored for that purpose.

**Run**

```sh
redstring                          # open the GUI; opens the case in the current directory, if any
redstring --check [case]           # validate and audit sourcing; exit 2 = errors, 1 = warnings, 0 = clean
redstring --derive [case]          # regenerate case.json, graph.md and dossiers
redstring --export [case]          # write the redacted <case>.redstring.zip bundle
redstring --shot [case] --out board.png   # headless render (also --width/--height, .json output)
redstring --debug                  # GUI with a debug log window
```

**Try it on the demo case.** `src/ui/testkit/fixtures/timeline_case/` is an
entirely invented case (a fictional town, `.example` domains, made-up
documents) used by the tests and for the screenshots above. Copy it somewhere
disposable and run `redstring` inside the copy:

```sh
cp -r src/ui/testkit/fixtures/timeline_case /tmp/redstring-demo
cd /tmp/redstring-demo && redstring
```

## Tests

```sh
cargo test                  # 717 tests, fully headless — never opens a window
cargo test ui::testkit      # scenario, round-trip, reconcile and lookup suites
```

The suite drives the application headlessly (no window, no sleeping); see
[TESTING.md](TESTING.md). `build.rs` additionally enforces a per-file size
ceiling and a no-silently-ignored-errors rule over non-test code.

## Limitations

This is unfinished software. Known gaps, roughly in order of how much they
matter:

- **No external review and no releases.** Everything here is a development
  preview; treat outputs as drafts to be checked, not as finished work.
- **Linux only so far.** Windows and macOS have not been built or tested.
- **The case format is version 1 and may change.** Records have no stable ids
  yet: entities are keyed by slug, so renaming an entity while another writer
  edits it becomes a conflict rather than an automatic merge, and a conflict is
  resolved for all listed records at once (yours or theirs), not item by item.
- **A case this binary cannot read** (for example one written by a newer
  version) holds all writes so nothing is overwritten — but if you have unsaved
  edits at that moment, Quit is refused. The way out today is Explore → quit
  without saving, which discards those edits.
- **The timeline is recomputed every frame**, so very large cases will be slow
  with it open; dense periods can overlap.
- **Lookups depend on external tools and services** you install and trust;
  their availability, rate limits and terms are outside this project.
- Cosmetic rough edges remain (for example, relations end at the square tile of
  an image node rather than at the image's outline).

## Licence

[0BSD](LICENSE) — public-domain-equivalent: use, change and redistribute it
without attribution.

Bundled flag images come from the public-domain
[region-flags](https://github.com/behdad/region-flags) set; see
[assets/flags/README.md](assets/flags/README.md) for sources and naming.
Third-party Rust crates are used under their own licences (see `Cargo.lock`).
