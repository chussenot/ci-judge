# ci-judge

Turn a merge request's evidence into a calibrated verdict file for a CI gate.

`ci-judge` is the judgment layer of a judgment gate. It collects what a
pipeline knows about a change (the Terraform plan, the JUnit reports, the
diff, the request's own fields, failed job traces), redacts secrets, shapes
the evidence into one request per item plus one global request, asks a
[`.jud` rubric](https://github.com/chussenot/judgment/blob/main/docs/reference/jud-format.md)
through the [`judgment`](https://crates.io/crates/judgment) crate, and
writes `judge-gate.verdict.json`. A gate reads that file and decides: on
GitLab a `judge-gate` block, on GitHub Actions `ci-judge gate` or the
composite action in this repository.

Nothing here generates text. A System One model answers typed questions
with calibrated probabilities; the rubric's policy turns them into
verdicts; the manifest beside the rubric turns verdicts into findings with
a file and a line. The model answers, code decides.

```
evidence ──collect──▶ state ──shape──▶ requests ──ask──▶ answers ──policy──▶ verdict file ──gate──▶ exit 0 / 1 / 2 / 3
```

## Install

A static binary per release, with a checksum:

```sh
curl -fsSLO https://github.com/chussenot/ci-judge/releases/download/v0.1.0/ci-judge-x86_64-unknown-linux-musl
curl -fsSLO https://github.com/chussenot/ci-judge/releases/download/v0.1.0/ci-judge-x86_64-unknown-linux-musl.sha256
sha256sum -c ci-judge-x86_64-unknown-linux-musl.sha256
install -m 755 ci-judge-x86_64-unknown-linux-musl ~/.local/bin/ci-judge
```

Or from source: `cargo install --git https://github.com/chussenot/ci-judge --locked`.

Or as a container, `FROM scratch`, the same binary, no shell, running as
uid 65532 with `/work` as the working directory:

```sh
docker run --rm -v "$PWD:/work" -e TYPESAFE_API_KEY ghcr.io/chussenot/ci-judge:0.1.0 run --rubric infra-plan --platform local
docker run --rm -v "$PWD:/work:ro" ghcr.io/chussenot/ci-judge:0.1.0 check my-rubric/
gh attestation verify oci://ghcr.io/chussenot/ci-judge:0.1.0 --repo chussenot/ci-judge
```

Every release pushes the version, its major.minor and `latest`, for
`linux/amd64` and `linux/arm64`, with a build-provenance attestation. The
image carries no `git`, so the `diff` source needs the diff collected as an
artifact (`git diff --merge-base BASE HEAD > changes.diff`, reduced with
`diffhunks`) when the judge runs inside it; the release binary in a job that
has `git` needs nothing. A job that runs *inside* an image (GitLab's Docker
executor) needs a shell the scratch image lacks: build a job image that
copies the binary out of this one.

## Backend

The same settings as the `jud` command, so a laptop and a runner read the
same thing: `TYPESAFE_API_KEY` and `TYPESAFE_BASE_URL` in the environment,
then `~/.config/jud/config.yaml` (`base_url`, `model`, `api_key`,
`timeout_secs`, `long_context`, `shared_images`), then the defaults. Any
server that speaks `POST /v1/systemone` is a base URL: the hosted API, a
local model behind `transformers serve` or Ollama, a gateway. `ci-judge
config` prints what a run would use and where each value came from; the
key is never printed.

## Run

```sh
ci-judge run --rubric infra-plan                      # writes judge-gate.verdict.json
ci-judge explain --rubric infra-plan                  # the requests and questions, no call
ci-judge collect --rubric infra-plan --dir requests   # the requests as files, no call
ci-judge run --rubric infra-plan --replay recordings  # answers from recordings, no key
```

`run` exits 0 when it wrote a verdict, 1 when a call or a collector failed,
2 when something was wrong before any call (no rubric, a manifest the
reader refuses, a model the rubric was not tuned on). It writes **no file**
when it cannot judge, so the gate sees a gate error and never a pass.

Three rubrics are bundled: `infra-plan` (one request per destructive
Terraform change, one over the plan), `mr-hygiene` (what, why, matches the
diff) and `failed-job-triage` (one request per failing test and failed
job). `ci-judge catalog` lists them; `--rubric path/to/manifest.yml` runs
your own. A repository's own manifest may not declare a `critical` or
`blocker` severity: a team does not edit the rubric that blocks its own
merges. Pass `--allow-local-blockers` to change that for a repository.

### Shadow mode

```sh
ci-judge run --rubric infra-plan --record recordings/infra-plan --cases-out recordings/infra-plan-cases.jud
```

Every answer is kept as a `.jud` Recording named after its request, and
every request is appended as an unlabelled case. Reviewers fill `expect`;
`ci-judge eval` then grades the recordings against the labels with the
crate's accuracy, Brier score and calibration error, proposes a threshold
per yes/no question, and exits 3 below the manifest's promotion bars.
`jud check`, `jud eval` and `jud tune` from the `judgment` crate read the
same files.

## Gate

```sh
ci-judge gate --mode advise      # shadow, advise, warn, block, route
```

Reads the verdict file, validates it the way the GitLab block does (an
invalid file is a gate error, never a pass), and applies the mode:

| Mode | What happens | Exit on `fail` |
|---|---|---|
| `shadow` | verdict stored, nothing posted | 0 |
| `advise` | one comment or note updated in place, annotations or a Code Quality report | 0 |
| `warn` | same, exit 3 | 3 |
| `block` | same, exit 1; the override label turns it into 3 | 1 |
| `route` | copies `<routes>/<route>.yml` to `judge-gate.route.yml`, writes `JUDGE_GATE_ROUTE` | 0 |

On GitHub it posts a pull request comment, prints `::error` / `::warning`
annotations on the files the findings name, appends to the step summary
and writes `verdict` and `route` to `GITHUB_OUTPUT`. On GitLab it writes
`gl-code-quality-judge-gate.json`, `judge-gate.env` and posts a merge
request note. Exit statuses are the block's: 0 pass, 1 blocked, 2 gate
error, 3 warning. Labels are read as the pipeline saw them, so an
override needs a new pipeline, not a job retry.

### GitHub Actions

```yaml
- uses: actions/checkout@v4
  with: { fetch-depth: 0 }
- uses: chussenot/ci-judge@v0.1.0
  with:
    rubric: mr-hygiene
    mode: advise
    fail-on: block        # block (default), closed, warn, never
  env:
    TYPESAFE_API_KEY: ${{ secrets.TYPESAFE_API_KEY }}
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

`examples/github-workflow.yml` has a two-job example with an artifact.

### GitLab CI

Run `ci-judge run` in the gate job's `before_script` and let the
`judge-gate` block's script decide; `examples/gitlab-ci.yml` shows the
install, the checksum and a fail-closed job. Where no block is available,
`ci-judge gate` in `script` does the same.

## The verdict file

```json
{
  "verdict": "fail",
  "summary": "2 failing check(s) over 2 destructive change(s); 0 undecided. ...",
  "route": null,
  "findings": [
    { "severity": "blocker", "description": "delete of aws_db_instance.main is not acknowledged in the merge request description (p=0.20, threshold 0.70)", "path": "infra/rds.tf", "line": 12 }
  ],
  "ci_judge": { "rubric": {...}, "model": "jev-1.14.0", "requests": 3, "evidence_hash": "sha256:…", "answers": [...] }
}
```

The first four fields are the gate's contract. `ci_judge` is the audit
trail: rubric and policy fingerprints, the model that answered, every
answer with the policy's reading, what was truncated, tokens and cost.

## Writing a rubric

A rubric is a plain `.jud` document the `judgment` crate reads, beside a
`manifest.yml` that the `.jud` reader would refuse as unknown fields:

```yaml
# manifest.yml
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
  destructive_ack: { request: change, severity: blocker, fails_when: "no", undecided_margin: 0.1,
                     finding: "{change.action} of {change.address} is not acknowledged in the merge request description" }
  rollback_obvious: { request: plan, severity: minor, fails_when: "no", finding: "Reverting would need manual steps that are not written down" }
```

The one rule that matters: **a question asked per item carries `when:
<item key>` in the rubric**, and a question asked only in the global
request carries `when: <state key>` or `when: <request name>`. A global
request's state also holds a key named after its request shape, so `when:
plan` or `when: pipeline` asks a question only there. A System One answer
carries no locator, so the request must: one request per item, the item
under a known key, and the finding template reads the item's fields.

Sources: `artifact` (a file or glob, reduced with `tfplan`, `junit`,
`json`, `text`, `lines`, `diffhunks`), `diff` (git, against the merge
base), `mr` (the change's fields), `file`, `env`, `joblog` (failed job
traces, GitLab). Reducers produce items with a path and a line where the
tool output has them; `tfplan` finds each resource's `.tf` file by
scanning the checkout.

## Developing

[mise](https://mise.jdx.dev) pins the tools and names the gates:

```sh
mise install            # rust 1.97.0 with rustfmt and clippy, cargo-deny, cargo-hack, cocogitto, jud, prek, gh
mise run ci             # fmt:check, lint, test, check, deny: what CI runs
mise run build:musl     # the static Linux binary
mise run image          # the container image as ghcr.io/chussenot/ci-judge:dev, from that binary
mise run judge:explain -- --rubric infra-plan   # what the model would see for this checkout
mise run jud:check      # the bundled rubrics through the judgment crate's own command line
```

`mise tasks` lists the rest. A `.env` beside the checkout (never committed)
holds `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL` and the platform tokens for
`mise run judge:run`. Commits are Conventional Commits; `cog bump --auto`
(`mise run release`) bumps the version from them, writes the changelog
section and tags, and the tag push runs the release workflow: the gates,
the three binaries, the GitHub release with checksums and provenance, then
the container image.

## Status

0.1.0. The judgment layer is complete for the three bundled rubrics; the
`joblog` source works on GitLab only; images as evidence and an
OpenTelemetry observer are the next steps. See `CHANGELOG.md`.

## License

MIT.
