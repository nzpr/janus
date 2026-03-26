# Decision: proxy pages deployment

## Task
TASK-PROXY-PAGES

## Date
2026-03-26

## Context
The repository now contains a static marketing/docs site for `codex-responses-api-proxy` under `site/`, and the user wants to publish it with GitHub Pages.

## Options Considered
- Move the site into a dedicated static-site toolchain and add a build step.
- Deploy the existing `site/` folder directly with a small GitHub Pages workflow.

## Decision
Deploy the existing `site/` folder directly with a path-scoped GitHub Pages workflow that stages only the public site files.

## Reasoning
The site is already plain static HTML/CSS/JS, so adding a site generator would increase moving parts without adding value. A direct Pages workflow is the smallest maintainable path. Staging only `index.html`, `main.js`, and `styles.css` avoids publishing local agent/session files that currently live under `site/`.

## Consequences
- Pages deployment stays simple and fast.
- Site changes only trigger the dedicated Pages workflow instead of unrelated CI.
- Local helper files inside `site/` are not published.
- The repository owner still needs GitHub Pages enabled to serve the deployed artifact.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260326-000000-proxy-pages.md](../../evolution/events/20260326-000000-proxy-pages.md)
- Evidence (files/tests):
  - `.github/workflows/proxy-pages.yml`
  - `site/index.html`
  - `site/.gitignore`
  - `site/main.js`
  - `site/styles.css`
  - `python3 -c "import yaml, pathlib; yaml.safe_load(pathlib.Path('.github/workflows/proxy-pages.yml').read_text())"`
