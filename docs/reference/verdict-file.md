---
title: Verdict file
description: The shape of judge-gate.verdict.json; the four fields a gate validates and acts on, and the audit trail ci-judge keeps under ci_judge.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, verdict, contract, reference]
---

# Verdict file

`judge-gate.verdict.json` is the seam between judging and deciding. A
gate validates it before reading it; an invalid file is a gate error
(exit 2), never a pass. ci-judge writes it whole or not at all.

## The contract

The four top-level fields a gate reads. The validation is the GitLab
block's jq, reproduced in `verdict::validate_loose` and applied by
`ci-judge gate`.

| Field | Type | Rule |
|---|---|---|
| `verdict` | `"pass"` or `"fail"` | required |
| `summary` | string | optional; one paragraph for the reviewers |
| `route` | string | optional; `[A-Za-z0-9_-]+`; route mode only |
| `findings` | array | optional; each `{description, severity?, path?, line?}` with a string `description` and a numeric `line`; `severity` one of `info`, `minor`, `major`, `critical`, `blocker`, anything else read as `major` |

Findings with a `path` reach the Code Quality report and are attached
to their file and line as GitHub annotations; every finding reaches the
note, the log and, on GitHub, an annotation.

## How ci-judge fills it

- `verdict` is `fail` when a question whose severity is in the manifest's
  `fail_on` landed on the failing side of its policy verdict. Undecided
  and deferred answers never fail.
- `summary` is the manifest's `summary` template, or a default built
  from counts: failing checks, requests, the worst finding.
- `route` is the chosen option of the `route_from` question, made
  file-safe, or absent.
- `findings` has one entry per failing question, with the item's
  locator, and one `info` entry per undecided answer, its description
  prefixed `undecided:`. A description holds no probability unless its
  template asks for `{p}`: the gates fingerprint `path:line:description`,
  and a number that moves between runs would make every finding new. The
  probability is in `ci_judge.answers`.

## The audit trail

Everything else is under `ci_judge`; a gate keeps it as an artifact and
never reads it.

```json
"ci_judge": {
  "version": "0.1.0",
  "rubric": { "name": "infra-plan", "version": "1", "fingerprint": "sha256:…", "policy_fingerprint": "sha256:…" },
  "model": "jev-1.14.0",
  "tuning_model": "jev-1.14.0",
  "requests": 3,
  "evidence_hash": "sha256:…",
  "truncation": { "items_dropped": {}, "keys_dropped": [] },
  "elapsed_ms": 4120,
  "input_tokens": 2310,
  "cost": null,
  "answers": [
    { "request": "mr-4821-change-aws_db_instance.main", "question": "destructive_ack",
      "policy": { "verdict": "no", "probability": 0.2 }, "outcome": "fail", "severity": "blocker" }
  ],
  "notes": []
}
```

| Field | Meaning |
|---|---|
| `rubric.fingerprint`, `policy_fingerprint` | The crate's fingerprints of the questions and of the gates, so a moved bar is as visible as a changed question |
| `model`, `tuning_model` | What answered, and what the bars were tuned on |
| `evidence_hash` | SHA-256 over every request's id and canonical state |
| `truncation` | Items dropped past a cap, keys dropped for the budget |
| `input_tokens`, `cost` | Summed over the requests; `cost` when the server reports one |
| `answers[]` | One per question per request: the policy's verdict as `Rubric::apply` wrote it, and the outcome `fail`, `pass`, `undecided` or `report` |
| `notes` | Optional sources left out, an untuned rubric, bars tuned on another model |

The gate copies the whole file to `judge-gate.verdict.json` beside its
own outputs, so the artifact a pipeline keeps carries the trail.
