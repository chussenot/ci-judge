---
title: Contributing
description: Where the developer documentation lives; the gates, hooks, harness and release runbook are under docs/project.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, contributing]
---

# Contributing

The developer pages are under `docs/project/`:

- [Contributing](docs/project/contributing.md): the checkout, the gates
  and the CI jobs that run them, the hooks, the documentation
  conventions, the Claude Code harness, cloud sessions.
- [Releasing](docs/project/releasing.md): from a merged `main` to the
  binaries and the image.
- [Architecture](docs/project/architecture.md): the C4 model.

In short: `mise install`, `mise run setup`, `mise run ci`; Conventional
Commits; never push a tag or an image by hand.
