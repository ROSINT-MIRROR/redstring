# Bundled flags

249 PNG flags, one per ISO 3166-1 alpha-2 code, compiled into the binary
(`src/model/flag.rs`, `include_bytes!`). Nothing is fetched at runtime.

## Source and licence

- **Images:** [region-flags](https://github.com/behdad/region-flags) 1.2.1, commit
  `c7f54514b5094f124e53c5e58c776c857f757a04` (2024-05-01). The upstream `COPYING`
  states the flags are public domain or otherwise exempt from copyright, taken from
  Wikimedia Commons. Several are public domain only under national law
  (`{{PD-AM-exempt}}` and similar): AM, AZ, JE, KG, KZ, MD, MX, MY, RS, YT. See the
  upstream `COPYING` and `html/` for per-flag provenance.
- **Proportions:** original aspect ratios as published upstream, not forced to 4:3 or
  1:1. Rasterised from upstream's 1024px PNGs to a 256px long side, then
  losslessly optimised with oxipng. Total: about 1.26 MB.
- **Names:** ISO 3166-1 English short names, as published by the ISO 3166 Maintenance
  Agency, transcribed from the Debian `iso-codes` 4.20.1 data file
  `iso_3166-1.json`. They are shown verbatim, with no regrouping or annotation.

## Scope

Only ISO 3166-1 alpha-2 codes are bundled. ISO 3166-1 lists both countries and other
territories, and the selector says so. Each entry is listed on its own, never grouped
under another. Upstream regions outside ISO 3166-1 (`XK`, `EU`, `UN`, `AC`, `CP`, `DG`,
`EA`, `IC`, `TA`, subdivision flags such as `US-CA`) are not bundled.

## Regenerate

    ./build-flags.sh <region-flags checkout> /usr/share/iso-codes/json/iso_3166-1.json

The script rewrites the PNGs and prints the `FlagMeta::ALL` rows for `src/model/flag.rs`.
