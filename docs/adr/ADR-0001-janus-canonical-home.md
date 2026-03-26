# ADR-0001: Move canonical fork home to janus

## Status
Accepted

## Date
2026-03-26

## Context
The proxy fork was being maintained and released from `nzpr/codex`, but the intended long-term home for the forked distribution, docs, Pages site, and release automation is `nzpr/janus`. Leaving `nzpr/codex` as the public home would keep package metadata, website links, releases, and operational ownership split across repositories.

## Decision
Make `nzpr/janus` the canonical repository for this fork. Preserve the prior `janus` default-branch content on a `legacy` branch, push the current fork state to `janus/main`, and update repo-facing links and package metadata to point at `nzpr/janus`.

## Options Considered
- Keep publishing from `nzpr/codex` and use `janus` only for separate experiments.
- Move the fork home to `nzpr/janus` and preserve the old repository state on a non-default branch.

## Consequences
### Positive
- Releases, Pages, source links, and package metadata all point to the same repository.
- The existing `janus` content remains recoverable on `legacy`.
- Future release and documentation work has a single canonical repo home.

### Negative
- Publishing automation may need to be re-authorized for the new repository in npm or GitHub settings.
- Existing links to `nzpr/codex` will need to be updated or tolerated until external references catch up.

## References
- Related task(s): repo migration to `nzpr/janus`
- Related decision notes:
- Related evolution events:
  - [20260326-073600-janus-repo-home.md](../../evolution/events/20260326-073600-janus-repo-home.md)
- Source links:
  - `README.md`
  - `addons/proxy/overlay/site/index.html`
  - `addons/proxy/overlay/codex-rs/responses-api-proxy/npm/README.md`
  - `addons/proxy/overlay/codex-rs/responses-api-proxy/npm/package.json`
