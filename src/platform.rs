//! Where the run is: a GitLab merge request pipeline, a GitHub Actions
//! pull request workflow, or a shell with neither. The platform gives the
//! change under review (title, description, labels, shas) and the API
//! calls the collectors and the gate need.

use std::path::PathBuf;

use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The merge or pull request under review, in one shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Change {
    pub number: Option<u64>,
    pub title: String,
    pub description: String,
    pub labels: Vec<String>,
    pub author: Option<String>,
    pub source_branch: Option<String>,
    pub target_branch: Option<String>,
    /// The head of the source branch.
    pub source_sha: Option<String>,
    /// The merge base, or the target's head when the merge base is unknown.
    pub base_sha: Option<String>,
    pub draft: bool,
    pub url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitLab {
    pub api_url: String,
    pub project_id: String,
    pub project_path: Option<String>,
    pub pipeline_id: Option<String>,
    pub job_name: Option<String>,
    pub job_url: Option<String>,
    pub mr_iid: Option<u64>,
    pub token: Option<String>,
    pub job_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitHub {
    pub api_url: String,
    /// `owner/repo`.
    pub repository: String,
    pub pr_number: Option<u64>,
    pub token: Option<String>,
    pub run_id: Option<String>,
    pub output_file: Option<PathBuf>,
    pub summary_file: Option<PathBuf>,
    pub job_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Platform {
    GitLab(GitLab),
    GitHub(GitHub),
    Local,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

impl Platform {
    /// Detect from the environment: `GITLAB_CI`, then `GITHUB_ACTIONS`,
    /// else local. `--platform` overrides the detection.
    pub fn detect(forced: Option<&str>) -> anyhow::Result<Self> {
        match forced {
            Some("gitlab") => return Ok(Self::GitLab(Self::gitlab_from_env())),
            Some("github") => return Self::github_from_env().map(Self::GitHub),
            Some("local" | "none") => return Ok(Self::Local),
            Some(other) => {
                return Err(anyhow!(
                    "unknown platform `{other}` (gitlab, github, local)"
                ));
            }
            None => {}
        }
        if env("GITLAB_CI").is_some() {
            return Ok(Self::GitLab(Self::gitlab_from_env()));
        }
        if env("GITHUB_ACTIONS").is_some() {
            return Self::github_from_env().map(Self::GitHub);
        }
        Ok(Self::Local)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::GitLab(_) => "gitlab",
            Self::GitHub(_) => "github",
            Self::Local => "local",
        }
    }

    fn gitlab_from_env() -> GitLab {
        GitLab {
            api_url: env("CI_API_V4_URL").unwrap_or_else(|| "https://gitlab.com/api/v4".to_owned()),
            project_id: env("CI_PROJECT_ID").unwrap_or_default(),
            project_path: env("CI_PROJECT_PATH"),
            pipeline_id: env("CI_PIPELINE_ID"),
            job_name: env("CI_JOB_NAME"),
            job_url: env("CI_JOB_URL"),
            mr_iid: env("CI_MERGE_REQUEST_IID").and_then(|v| v.parse().ok()),
            token: env("GITLAB_TOKEN"),
            job_token: env("CI_JOB_TOKEN"),
        }
    }

    fn github_from_env() -> anyhow::Result<GitHub> {
        let pr_number = env("GITHUB_EVENT_PATH")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|e| e.pointer("/pull_request/number").and_then(Value::as_u64));
        Ok(GitHub {
            api_url: env("GITHUB_API_URL").unwrap_or_else(|| "https://api.github.com".to_owned()),
            repository: env("GITHUB_REPOSITORY").unwrap_or_default(),
            pr_number,
            token: env("GITHUB_TOKEN"),
            run_id: env("GITHUB_RUN_ID"),
            output_file: env("GITHUB_OUTPUT").map(PathBuf::from),
            summary_file: env("GITHUB_STEP_SUMMARY").map(PathBuf::from),
            job_name: env("GITHUB_JOB"),
        })
    }

    /// A short, file-safe name for this run's requests and cases:
    /// `mr-123`, `pr-45`, `local`.
    pub fn slug(&self) -> String {
        match self {
            Self::GitLab(gl) => gl
                .mr_iid
                .map_or_else(|| "pipeline".to_owned(), |iid| format!("mr-{iid}")),
            Self::GitHub(gh) => gh
                .pr_number
                .map_or_else(|| "run".to_owned(), |n| format!("pr-{n}")),
            Self::Local => "local".to_owned(),
        }
    }

    /// The labels on the change, as the pipeline saw them when it started.
    pub fn labels(&self) -> Vec<String> {
        match self {
            Self::GitLab(_) => env("CI_MERGE_REQUEST_LABELS")
                .map(|l| {
                    l.split(',')
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            Self::GitHub(_) => github_event()
                .and_then(|e| e.pointer("/pull_request/labels").cloned())
                .and_then(|l| l.as_array().cloned())
                .map(|labels| {
                    labels
                        .iter()
                        .filter_map(|l| l.get("name").and_then(Value::as_str).map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            Self::Local => Vec::new(),
        }
    }

    /// The change under review, from the environment alone (no call).
    pub fn change(&self) -> Change {
        match self {
            Self::GitLab(gl) => Change {
                number: gl.mr_iid,
                title: env("CI_MERGE_REQUEST_TITLE").unwrap_or_default(),
                description: env("CI_MERGE_REQUEST_DESCRIPTION").unwrap_or_default(),
                labels: self.labels(),
                author: env("GITLAB_USER_LOGIN"),
                source_branch: env("CI_MERGE_REQUEST_SOURCE_BRANCH_NAME"),
                target_branch: env("CI_MERGE_REQUEST_TARGET_BRANCH_NAME"),
                source_sha: env("CI_COMMIT_SHA"),
                base_sha: env("CI_MERGE_REQUEST_DIFF_BASE_SHA")
                    .or_else(|| env("CI_MERGE_REQUEST_TARGET_BRANCH_SHA")),
                draft: env("CI_MERGE_REQUEST_TITLE")
                    .is_some_and(|t| t.starts_with("Draft:") || t.starts_with("WIP:")),
                url: gl
                    .project_path
                    .as_ref()
                    .zip(gl.mr_iid)
                    .and_then(|(path, iid)| {
                        env("CI_SERVER_URL")
                            .map(|server| format!("{server}/{path}/-/merge_requests/{iid}"))
                    }),
            },
            Self::GitHub(gh) => {
                let event = github_event().unwrap_or(Value::Null);
                let pr = event.get("pull_request").cloned().unwrap_or(Value::Null);
                let s =
                    |v: &Value, p: &str| v.pointer(p).and_then(Value::as_str).map(str::to_owned);
                Change {
                    number: gh.pr_number,
                    title: s(&pr, "/title").unwrap_or_default(),
                    description: s(&pr, "/body").unwrap_or_default(),
                    labels: self.labels(),
                    author: s(&pr, "/user/login"),
                    source_branch: s(&pr, "/head/ref"),
                    target_branch: s(&pr, "/base/ref"),
                    source_sha: s(&pr, "/head/sha").or_else(|| env("GITHUB_SHA")),
                    base_sha: s(&pr, "/base/sha"),
                    draft: pr
                        .pointer("/draft")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    url: s(&pr, "/html_url"),
                }
            }
            Self::Local => Change {
                title: env("CI_JUDGE_TITLE").unwrap_or_default(),
                description: env("CI_JUDGE_DESCRIPTION").unwrap_or_default(),
                base_sha: env("CI_JUDGE_BASE"),
                ..Change::default()
            },
        }
    }
}

fn github_event() -> Option<Value> {
    env("GITHUB_EVENT_PATH")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
}

/// A failed job of the current pipeline, with the tail of its trace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FailedJob {
    pub id: u64,
    pub name: String,
    pub stage: Option<String>,
    pub trace_tail: String,
    pub web_url: Option<String>,
}

impl GitLab {
    fn client(&self) -> anyhow::Result<reqwest::Client> {
        reqwest::Client::builder()
            .user_agent(concat!("ci-judge/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("http client")
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> anyhow::Result<reqwest::RequestBuilder> {
        if let Some(token) = &self.token {
            Ok(req.header("PRIVATE-TOKEN", token))
        } else if let Some(job) = &self.job_token {
            Ok(req.header("JOB-TOKEN", job))
        } else {
            Err(anyhow!(
                "no GITLAB_TOKEN and no CI_JOB_TOKEN for the GitLab API"
            ))
        }
    }

    fn project(&self) -> String {
        reqwest::Url::parse(&format!("http://x/{}", self.project_id))
            .ok()
            .and_then(|u| {
                u.path_segments()
                    .and_then(|mut s| s.next().map(str::to_owned))
            })
            .unwrap_or_else(|| self.project_id.clone())
    }

    /// The whole description of the merge request. GitLab cuts
    /// `CI_MERGE_REQUEST_DESCRIPTION` at 2700 characters and says so in
    /// `CI_MERGE_REQUEST_DESCRIPTION_IS_TRUNCATED`; the API does not.
    pub async fn description(&self) -> anyhow::Result<String> {
        let iid = self
            .mr_iid
            .ok_or_else(|| anyhow!("no CI_MERGE_REQUEST_IID"))?;
        let url = format!(
            "{}/projects/{}/merge_requests/{iid}",
            self.api_url,
            self.project()
        );
        let mr: Value = self
            .auth(self.client()?.get(&url))?
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .context("reading the merge request")?
            .json()
            .await
            .context("decoding the merge request")?;
        Ok(mr
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned())
    }

    /// The failed jobs of this pipeline with the last `tail_lines` of each
    /// trace. Reads jobs with the job token where it can; traces need
    /// `GITLAB_TOKEN` on most instances.
    pub async fn failed_jobs(&self, tail_lines: usize) -> anyhow::Result<Vec<FailedJob>> {
        let pipeline = self
            .pipeline_id
            .as_ref()
            .ok_or_else(|| anyhow!("no CI_PIPELINE_ID"))?;
        let client = self.client()?;
        let url = format!(
            "{}/projects/{}/pipelines/{pipeline}/jobs?scope[]=failed&per_page=100",
            self.api_url,
            self.project()
        );
        let jobs: Vec<Value> = self
            .auth(client.get(&url))?
            .send()
            .await
            .context("listing failed jobs")?
            .error_for_status()
            .context("listing failed jobs")?
            .json()
            .await
            .context("decoding the job list")?;
        let mut out = Vec::new();
        for job in jobs {
            let id = job.get("id").and_then(Value::as_u64).unwrap_or_default();
            let name = job
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            if self.job_name.as_deref() == Some(name.as_str()) {
                continue;
            }
            let trace_url = format!(
                "{}/projects/{}/jobs/{id}/trace",
                self.api_url,
                self.project()
            );
            let trace = self
                .auth(client.get(&trace_url))?
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .context("reading a job trace")?
                .text()
                .await
                .unwrap_or_default();
            out.push(FailedJob {
                id,
                name,
                stage: job.get("stage").and_then(Value::as_str).map(str::to_owned),
                trace_tail: tail(&strip_ansi(&trace), tail_lines),
                web_url: job
                    .get("web_url")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
        }
        Ok(out)
    }

    /// Create or update the one note carrying `marker` on the merge request.
    pub async fn upsert_note(&self, marker: &str, body: &str) -> anyhow::Result<&'static str> {
        let iid = self
            .mr_iid
            .ok_or_else(|| anyhow!("no CI_MERGE_REQUEST_IID"))?;
        let token = self
            .token
            .as_ref()
            .ok_or_else(|| anyhow!("no GITLAB_TOKEN"))?;
        let client = self.client()?;
        let notes_url = format!(
            "{}/projects/{}/merge_requests/{iid}/notes",
            self.api_url,
            self.project()
        );
        let existing: Vec<Value> = client
            .get(format!("{notes_url}?per_page=100"))
            .header("PRIVATE-TOKEN", token)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .context("listing notes")?
            .json()
            .await
            .unwrap_or_default();
        let note_id = existing
            .iter()
            .find(|n| {
                n.get("body")
                    .and_then(Value::as_str)
                    .is_some_and(|b| b.contains(marker))
            })
            .and_then(|n| n.get("id").and_then(Value::as_u64));
        let payload = serde_json::json!({ "body": body });
        let (req, action) = match note_id {
            Some(id) => (client.put(format!("{notes_url}/{id}")), "updated"),
            None => (client.post(&notes_url), "posted"),
        };
        req.header("PRIVATE-TOKEN", token)
            .json(&payload)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .context("writing the merge request note")?;
        Ok(action)
    }
}

impl GitHub {
    fn client(&self) -> anyhow::Result<reqwest::Client> {
        reqwest::Client::builder()
            .user_agent(concat!("ci-judge/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("http client")
    }

    /// Create or update the one issue comment carrying `marker` on the
    /// pull request.
    pub async fn upsert_comment(&self, marker: &str, body: &str) -> anyhow::Result<&'static str> {
        let number = self
            .pr_number
            .ok_or_else(|| anyhow!("not a pull request event"))?;
        let token = self
            .token
            .as_ref()
            .ok_or_else(|| anyhow!("no GITHUB_TOKEN"))?;
        let client = self.client()?;
        let base = format!(
            "{}/repos/{}/issues/{number}/comments",
            self.api_url, self.repository
        );
        let existing: Vec<Value> = client
            .get(format!("{base}?per_page=100"))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .context("listing comments")?
            .json()
            .await
            .unwrap_or_default();
        let id = existing
            .iter()
            .find(|c| {
                c.get("body")
                    .and_then(Value::as_str)
                    .is_some_and(|b| b.contains(marker))
            })
            .and_then(|c| c.get("id").and_then(Value::as_u64));
        let payload = serde_json::json!({ "body": body });
        let (req, action) = match id {
            Some(id) => (
                client.patch(format!(
                    "{}/repos/{}/issues/comments/{id}",
                    self.api_url, self.repository
                )),
                "updated",
            ),
            None => (client.post(&base), "posted"),
        };
        req.bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&payload)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .context("writing the pull request comment")?;
        Ok(action)
    }
}

/// The last `lines` lines of a text.
pub fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

/// Strip ANSI escape sequences and GitLab section markers from a trace.
pub fn strip_ansi(text: &str) -> String {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\[0K|section_(?:start|end):\d+:[A-Za-z0-9_]+\r?",
        )
        .unwrap()
    });
    RE.replace_all(text, "").replace('\r', "")
}
