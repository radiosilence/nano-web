Released 2026-07-26.

### Changed

- **Docker**: The image has no build stage at all now — it is a single `FROM scratch` that copies in the musl binary CI already built. Building it by hand requires `dist/` populated first; docker builds only ever happen in CI, so the source-build path was carrying no weight.
- **CI**: Docker images no longer recompile the binary. `build-image` now owns the linux musl compile and copies the result straight into a `scratch` image on the same runner, then uploads it as the release asset — removing two full `lto = "fat"` compiles per push.
- **CI**: Dropped the GHA layer cache on the Docker builds. With no compile in the image there is nothing worth caching, and `mode=max` was filling the repo-wide 10GB cache that `Swatinem/rust-cache` shares.
- **CI**: `test` and `lint` only save caches on `main`. Branch-scoped caches can't be read by other branches, so PR runs were writing entries that only evicted the ones they restore from.
- **CI**: Dropped the redundant cache on the formatting job — `cargo fmt --check` compiles nothing.
- **CI**: PRs now run the full build chain — every target binary plus both Docker images. Only the registry push, manifest, release and crate publish stay gated on `main`. A broken `Dockerfile` or a target that stops compiling now fails before merge instead of after.

### Removed

- **CI**: The `quick-test` Docker smoke test job. Release now gates on `create-manifest` so the tag still lands only after the images are pushed.
