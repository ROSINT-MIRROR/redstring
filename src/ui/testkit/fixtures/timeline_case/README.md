# Synthetic timeline demo case

Everything in this directory is invented for the Red String timeline demo:
the town of Eastbrook, its people, Brightwater Paving SRL, the `.example`
domains and every file under `docs/`. No real person, company or record is
described. Do not cite it.

What it exercises:

- event dates at day, month and year precision, a closed period, an open-ended period
- a time of day with an explicit UTC offset (`+03:00`), one in UTC (`Z`) and one with no zone
- publication dates on sources, distinct from capture (`fetched_at`) and case-entry (`created_at`) times
- two conflicting accounts of the same fact (incorporated 2018-03 vs 2017)
- an inferred, unsourced hypothesis; a rejected claim; a claim denied by its target
- a relation with no entry stamp (`created_at: 0`), and records with no date at all
