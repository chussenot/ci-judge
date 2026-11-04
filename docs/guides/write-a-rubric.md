---
title: Write a rubric
description: How to write a rubric for ci-judge in its three files, the manifest, the .jud rubric and the .jud cases, with the questions shaped so that a finding names a file and a line.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, rubric, jud, how-to]
---

# Write a rubric

**You will** have a directory a pipeline can judge with. **Prerequisites:**
[The granularity rule](../concepts/granularity.md); the
[jud format](https://github.com/chussenot/judgment/blob/main/docs/reference/jud-format.md)
for the two `.jud` files; `jud` on PATH helps (`cargo install judgment
--features cli`).

A rubric is three files in one directory:

| File | Read by | Holds |
|---|---|---|
| `manifest.yml` | ci-judge only | Where the evidence comes from, how requests are shaped, what each answer means in CI terms |
| `<name>.jud` | the `judgment` crate, `jud`, ci-judge | The questions, their criteria, the policy's bars, the tuning provenance |
| `<name>-cases.jud` | the same | Labelled states, grown by shadow mode |

The split exists because the `.jud` reader refuses any field it does not
know, by path. Severities, finding templates and promotion bars would be
refused inside the rubric, so they live beside it.

## 1. Name the items

Decide what one finding is about: a resource change, a failing test, a
changed file. The reducer that yields it fixes the fields a question can
read and the locator a finding gets ([Sources and
reducers](../reference/collectors.md)). If no reducer yields your item,
that is a Rust change before a rubric.

## 2. Write the questions

Per item first, in the `.jud`:

```yaml
spec:
  questions:
    destructive_ack:
      type: noul
      when: change                              # asked once per item request
      instructions: >-
        Does `mr.description` explicitly acknowledge the `change.action` of
        the resource `change.address` and give a reason for it?
      criteria:
        true: the description names this resource, says it will be destroyed or replaced, and gives a reason
        false: the description does not mention it, or gives no reason
    rollback_obvious:
      type: noul
      when: plan                                # asked once, in the global request
      instructions: >-
        Would reverting this merge request restore the previous state of
        `plan.changes` without manual steps?
      criteria:
        true: every change is reversible by re-applying the previous configuration
        false: at least one change loses state a revert cannot bring back
```

Three habits make a question answerable: name the state keys it reads in
backticks; ask one fact per question; write `criteria` that say what each
answer means in the evidence's own terms. A question that needs a reason
is two questions.

The policy holds one gate per question with a `note`:

```yaml
  policy:
    destructive_ack:
      threshold: 0.7
      note: untuned; a blocker starts strict and `jud tune` moves it
```

No `tuning` block until `jud tune` produced one on recordings.

## 3. Write the manifest

```yaml
name: infra-plan
rubric: infra-plan.jud
cases: infra-plan-cases.jud
state:
  - { key: plan, source: artifact, path: tfplan.json, reduce: tfplan, priority: 1 }
  - { key: mr, source: mr, priority: 2 }
requests:
  - { name: change, items: plan, item_key: change, filter: { action: [delete, replace] }, max_items: 25 }
  - { name: plan }
questions:
  destructive_ack:
    request: change
    severity: blocker
    fails_when: "no"
    undecided_margin: 0.1
    finding: "{change.action} of {change.address} is not acknowledged in the merge request description"
  rollback_obvious:
    request: plan
    severity: minor
    fails_when: "no"
    finding: "Reverting the merge request would need manual steps that are not written down"
promotion:
  accuracy_at_least: { destructive_ack: 0.9 }
  labelled_at_least: 20
```

Every field is in [Manifest](../reference/manifest.md). A repository's own
manifest may not declare `critical` or `blocker`: a team does not edit the
rubric that blocks its own merges. Only the bundled catalog may.

## 4. Write a few cases

Synthetic states in the exact shape a request has, with `expect`, so
the three files bind and `eval` has something to grade before shadow mode
fills the real ones:

```yaml
apiVersion: jud/v1
kind: Cases
metadata: { name: infra-plan-cases, minor: 4 }
spec:
  rubric: infra-plan
  cases:
    - id: silent-delete
      state:
        change: { address: aws_db_instance.main, action: delete }
        mr: { title: "Tidy up", description: "Refactor variable names." }
      expect: { destructive_ack: false }
```

## 5. Check and look

```sh
ci-judge check path/to/rubric
ci-judge explain --rubric path/to/rubric --platform local   # on a fixture in the working directory
jud check path/to/rubric/*.jud
```

`check` refuses a per-item question without `when: <item key>`, a
manifest question the rubric lacks, a severity the gate does not know.
`explain` shows each request and the questions its `when` left in; a
request that asks nothing is a `when` naming a key the state lacks.

## Next

[Shadow, label, eval, promote](shadow-and-eval.md): the recordings and
labels that give the policy its bars.
