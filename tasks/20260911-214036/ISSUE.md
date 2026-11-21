Add a CI pipeline to run and test my code.

From repo: https://github.com/jonhoo/rust-ci-conf
Use this as an example: https://github.com/Vaengir-Projects/rigit/pull/6

# Changes

CI is based on [jonhoo/rust-ci-conf](https://github.com/jonhoo/rust-ci-conf), merged in as its own history (see
`.github/DOCS.md`), so template updates can be pulled with `git merge ci/main`.

- `check`: fmt, clippy (stable and beta), docs and the MSRV (1.87, also set as `rust-version`).
- `test`: the tests on stable and beta, on macOS, and with coverage uploaded to Codecov.
- `rolling`: daily runs on nightly and with updated dependencies.
- Dependabot bundles minor and patch updates of the crates into one PR per week.
- A ruleset on `main` requires the stable checks. When the MSRV changes, update the required `ubuntu / <msrv>` check.

Removed from the template because they don't fit a binary crate: `safety`, `nostd`, `semver` (no release on crates.io
yet), `hack` (no features) and `minimal` (minimal versions).
