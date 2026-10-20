Released 2026-07-27.

### Fixed

- **Dev mode**: `--dev` now reloads the URLs people actually load. Index files are published under two keys — `/index.html` and `/` — but the refresh only re-inserted the canonical one, so editing `index.html` and reloading `/` served the bytes captured at startup, indefinitely. Route publishing now goes through one function shared by the startup scan and the refresh, so the alias cannot be left behind again.
- **Dev mode**: Files created after startup are picked up instead of 404ing forever. The refresh returned early when a URL had no route, which is exactly the case for a new file.
- **Dev mode**: Deleted files stop being served instead of returning their last-known contents. The missing-file error was discarded by the caller, leaving the stale route in place.

Production serving is unaffected — routes there are write-once by design.
