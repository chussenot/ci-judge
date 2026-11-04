---
title: First run
description: A tutorial that judges a fixture Terraform plan with the bundled infra-plan rubric, first with no model (explain), then with one (run), then applies a gate mode locally.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, tutorial, getting-started]
---

# First run

**You will** see what the model would be asked about a change, ask it,
and apply a gate mode to the answer, all from a shell. **Prerequisites:**
[Install](install.md); for the second step, a System One backend (a
`TYPESAFE_API_KEY`, or a `TYPESAFE_BASE_URL` to any server speaking
`POST /v1/systemone`).

## 1. A fixture

A Terraform plan with one deletion and one replacement, and the `.tf`
file the resources live in, so findings get a path and a line:

```sh
mkdir -p first-run/infra && cd first-run
cat > tfplan.json <<'EOF2'
{"format_version":"1.2","resource_changes":[
 {"address":"aws_db_instance.main","type":"aws_db_instance","name":"main","change":{"actions":["delete"],"before":{"id":"db"},"after":null}},
 {"address":"aws_security_group.app","type":"aws_security_group","name":"app","change":{"actions":["delete","create"],"before":{},"after":{}}}
]}
EOF2
printf 'resource "aws_db_instance" "main" {}\n\nresource "aws_security_group" "app" {}\n' > infra/main.tf
```

Outside a pipeline there is no merge request, so the change's title and
description come from two variables:

```sh
export CI_JUDGE_TITLE="Move the database"
export CI_JUDGE_DESCRIPTION="Drops aws_db_instance.main; a snapshot was taken on 2026-11-01."
```

## 2. What the model would see, no call

```sh
ci-judge explain --rubric infra-plan --platform local
```

Three requests: one per destructive change, with the change under
`change` and the question `destructive_ack`; one over the whole plan
with `rollback_obvious` and `blast_radius`. Each request shows its
locator (`infra/main.tf:1`), its size in bytes and its state. That is the
whole contract with the model: a JSON state and typed questions.

## 3. Ask the model

```sh
export TYPESAFE_API_KEY=...
ci-judge run --rubric infra-plan --platform local
```

The bundled rubric carries no `tuning` block yet, so the run notes that
its bars are untuned defaults and goes on; a rubric tuned on another
model would stop instead, unless `--allow-untuned` is passed. The run logs each source it collected, the
backend it asks, and ends with the verdict and `judge-gate.verdict.json`
beside you. Open it: `verdict`, a `summary`, `findings` with the file and
line of each unacknowledged change, and `ci_judge` with every answer and
the bar it was read against.

Without a key the run stops before any call with status 2 and a message
naming the variable; it writes no file.

## 4. Apply a mode

```sh
ci-judge gate --mode advise --platform local --out gate
ci-judge gate --mode block --platform local --out gate; echo "exit $?"
```

`advise` writes the note a reviewer would see (`gate/judge-gate.note.md`)
and the Code Quality report; `block` exits 1 on a `fail`. The five modes
are in [Gate](../reference/gate.md).

## Next

- [Run in GitLab CI](../guides/gitlab.md) or
  [Run in GitHub Actions](../guides/github.md): the same two commands in
  a job.
- [Shadow, label, eval, promote](../guides/shadow-and-eval.md): how a
  rubric earns its bars.
