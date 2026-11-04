---
title: The granularity rule
description: Why ci-judge asks a question once per evidence item, how a request is shaped, what the `when` declaration does, and what this costs.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, concepts, requests, rubric]
---

# The granularity rule

A System One answer is a probability. It carries no reason and no
locator. Everything a reviewer reads in a finding, the file, the line,
the resource address, must therefore come from how the request was
shaped, not from the answer.

**One request per evidence item, the item under one state key, the
question gated on that key.** A blocker is always asked this way, so its
finding has a path and a line. Questions that need the whole picture are
asked once, in a global request, and may not block.

## How a request is shaped

The manifest names state sources; each source's reducer yields a value
and, where the output is a list of things, items with a locator
([Sources and reducers](../reference/collectors.md)). The manifest then
names request shapes:

- An **item shape** takes items from one source, filters them, caps them,
  and puts each item under `item_key`. Its state is every global source
  except the item sources, plus that one item. `infra-plan`'s `change`
  shape: `{change: <one resource change>, mr: {...}, runbook: "..."}`.
- The **global shape** takes everything, item sources included, and adds
  one key named after itself (`plan: {...}` when no source already uses
  that name), so a rubric can say `when: plan`.

Item requests carry no item source at all, which is what keeps a global
question's `when` from matching them.

## What `when` does

The `.jud` reader (`Rubric::lower`) asks a question only when its `when`
path is present in the state. One rubric therefore serves both shapes:

```yaml
destructive_ack:
  when: change      # asked in every item request, never in the global one
rollback_obvious:
  when: plan        # asked once, in the global request
```

`ci-judge check` refuses a manifest whose per-item question lacks `when:
<item key>`, and warns when a blocker is global.

## What it costs

N items are N+1 calls. Each is small (a few hundred to a few thousand
input tokens) and produces no output tokens, and the crate paces them.
The manifest's `max_items` and the `--max-items` flag cap N; past the cap
the lowest-priority items are dropped and the verdict says
`truncation.items_dropped`. A rubric whose items routinely hit the cap
has the wrong item: filter the source, or ask about files instead of
hunks.

## What it buys

- A finding the Code Quality widget and GitHub annotations can place.
- A case per item in shadow mode, so labels are about one thing.
- Recordings keyed by one item's state, so a changed item re-asks only
  itself.
