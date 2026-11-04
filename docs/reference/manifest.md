---
title: Manifest
description: Every field of manifest.yml, the file beside a .jud rubric that holds what the .jud reader would refuse; state sources, request shapes, per-question severities and findings, fail_on, route_from, summary, promotion.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, manifest, rubric, reference]
---

# Manifest

`manifest.yml` sits beside a `.jud` rubric and is read by ci-judge only.
It holds everything CI-specific that the `.jud` reader would refuse as an
unknown field. An unknown field here is refused too (`check` and `run`
exit 2 naming it). `src/manifest.rs` is the definition; this page is the
reading.

## Top level

| Field | Required | Meaning |
|---|---|---|
| `name` | yes | The rubric's name: the catalog key for a bundled rubric; not checked against the `.jud`'s `metadata.name` |
| `owner`, `description` | no | For the reader |
| `rubric` | yes | The `.jud` Rubric, relative to the manifest |
| `cases` | no | The `.jud` Cases, relative; grown by `--cases-out` |
| `recordings` | no, `recordings` | The recordings directory, relative; `eval`'s default `--replay` |
| `state` | yes | The sources, below |
| `requests` | yes | The request shapes, below |
| `questions` | yes | Per question, below |
| `fail_on` | no, `[blocker, critical]` | The severities that make the verdict `fail`; the rest only produce findings |
| `route_from` | no | `{ question, fallback }`: the Choice whose chosen option names the route; `fallback` when it is deferred |
| `summary` | no | A template for the verdict's summary: `{failed}`, `{undecided}`, `{findings}`, `{requests}`, `{items}`, `{worst}` |
| `promotion` | no | The bars `eval` holds, below |

## `state[]`

| Field | Required | Meaning |
|---|---|---|
| `key` | yes | The state key the value lands under; questions name it in backticks |
| `source` | yes | `artifact`, `diff`, `mr`, `file`, `env`, `joblog` ([Sources and reducers](collectors.md)) |
| `path` | for `artifact`, `file`, `env` | A path or glob, or a variable name; `${VAR}` expands from the environment |
| `reduce` | no | `tfplan`, `junit`, `diffhunks`, `json`, `text`, `lines`; each source has a default |
| `priority` | no, 5 | Lower is kept longer when the byte budget drops keys |
| `optional` | no, false | A source that cannot be collected is left out with a note instead of failing the run |
| `tail_lines` | no | Lines kept from the end, for `text` and `joblog` (200) |

## `requests[]`

| Field | Required | Meaning |
|---|---|---|
| `name` | yes | The shape's name; a global request's state also carries a key of this name |
| `items` | no | The state key whose items each become a request; absent for the global request |
| `item_key` | no, the name | The key one item lands under |
| `filter` | no | `{ field: [values] }`: keep items whose field has one of the values |
| `max_items` | no | Cap; past it the verdict records `truncation.items_dropped` |

An item request carries every global source except the item sources,
plus its item. The global request carries everything.

## `questions.<id>`

The id must exist in the rubric.

| Field | Required | Meaning |
|---|---|---|
| `request` | yes | The shape it is asked in; per item, the rubric must declare `when: <item_key>` |
| `severity` | no, `major` | `info`, `minor`, `major`, `critical`, `blocker` (the Code Quality levels) |
| `fails_when` | unless `report_only` | What a failing answer is: `"yes"` or `"no"` for a Noul; `reached` or `not_reached` for a Score with `level_at_least`; `{ option: [keys] }` for a Choice |
| `undecided_margin` | no | A Noul within this distance of its threshold is undecided: an `info` finding, never a fail; `[0, 0.5)` |
| `finding` | no | The finding's text; `{key.path}` reads the request's state, `{p}`, `{threshold}`, `{confidence}`, `{level}`, `{option}`, `{verdict}` the answer |
| `report_only` | no, false | Recorded in the verdict, never a finding |

A deferred Choice or Score (below its confidence bar) is undecided
whatever `fails_when` says.

## `promotion`

| Field | Meaning |
|---|---|
| `accuracy_at_least` | `{ question: 0.9 }` or `{ "*": 0.9 }` |
| `ece_at_most` | expected calibration error, same keys |
| `labelled_at_least` | labelled cases per graded question |

## Severity cap

A manifest that is not in the bundled catalog may not declare `critical`
or `blocker`: `run` exits 2 unless `--allow-local-blockers`. The catalog
is compiled into the binary; changing it is a release.
