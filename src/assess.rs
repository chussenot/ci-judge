//! The backend a run asks, behind the crate's `SystemOne` trait: the
//! configured client, a `Replay` over recordings, or either wrapped in a
//! `Recorder` for shadow mode. Tests hand in a `Fake`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use judgment::backend::{Paced, Pacer};
use judgment::eval::canonical::request_fingerprint;
use judgment::eval::{Recording, now_rfc3339};
use judgment::jud::{Rubric, recording_to_yaml};
use judgment::{Questions, Replay, Response, SystemOne};
use serde_json::Value;

use crate::config::Backend;
use crate::error::{Exit, Failure, Result};
use crate::requests::Request;

/// How a run answers its requests.
pub struct Assessor {
    backend: Arc<dyn SystemOne>,
    pub model: String,
    pub describe: String,
    pub record_dir: Option<PathBuf>,
    pub rubric_name: String,
    pub server: Option<String>,
    pub max_in_flight: usize,
}

/// One answered request.
#[derive(Debug, Clone)]
pub struct Answered {
    pub request: Request,
    pub asked: Questions,
    pub response: Response,
    pub elapsed_ms: u64,
}

impl Assessor {
    /// A replay over `dir`: no key, no network.
    pub fn replay(dir: &Path, model: &str, rubric: &str) -> Result<Self> {
        let replay = Replay::open(dir)
            .map_err(|e| Failure::usage(format!("replay {}: {e}", dir.display())))?;
        Ok(Self {
            backend: Arc::new(replay),
            model: model.to_owned(),
            describe: format!("replay of {} ({} recordings)", dir.display(), 0),
            record_dir: None,
            rubric_name: rubric.to_owned(),
            server: None,
            max_in_flight: 8,
        })
    }

    /// The configured client, paced.
    pub fn client(backend: &Backend, rubric: &str, max_in_flight: usize) -> Result<Self> {
        let pacer = Pacer::new(max_in_flight);
        let client = backend.client()?;
        Ok(Self {
            backend: Arc::new(Paced::new(client, pacer)),
            model: backend.model.clone(),
            describe: format!(
                "{} at {} (model from {}, base URL from {}, key from {})",
                backend.model,
                backend.public_url(),
                backend.model_from,
                backend.base_url_from,
                backend.api_key_from
            ),
            record_dir: None,
            rubric_name: rubric.to_owned(),
            server: Some(backend.public_url()),
            max_in_flight,
        })
    }

    /// Any backend, for tests.
    pub fn with_backend(backend: impl SystemOne + 'static, model: &str, rubric: &str) -> Self {
        Self {
            backend: Arc::new(backend),
            model: model.to_owned(),
            describe: "a test backend".to_owned(),
            record_dir: None,
            rubric_name: rubric.to_owned(),
            server: None,
            max_in_flight: 4,
        }
    }

    /// Record every answer under `dir` as a `.jud` Recording named after
    /// its request id, the way `jud record` names a case's recording, so
    /// `jud check`, `jud eval` and `Replay` all find it.
    pub fn recording_to(mut self, dir: impl Into<PathBuf>) -> Self {
        self.record_dir = Some(dir.into());
        self
    }

    /// Answer every request, bounded by the pacer, in order of the input.
    pub async fn answer_all(
        &self,
        rubric: &Rubric,
        requests: Vec<Request>,
    ) -> Result<Vec<Answered>> {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_in_flight.max(1)));
        let mut handles = Vec::with_capacity(requests.len());
        for request in requests {
            let asked = rubric
                .lower(&request.state, &Default::default())
                .map_err(|e| Failure::usage(format!("lowering request {}: {e}", request.id)))?;
            if asked.is_empty() {
                continue; // no question has its `when` path in this state: nothing to ask
            }
            let backend = Arc::clone(&self.backend);
            let model = self.model.clone();
            let permit = Arc::clone(&semaphore);
            handles.push(tokio::spawn(async move {
                let _permit = permit.acquire_owned().await;
                let started = Instant::now();
                let outcome = backend.answer(&request.state, &model, &asked).await;
                let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                (request, asked, outcome, elapsed_ms)
            }));
        }
        let mut answered = Vec::with_capacity(handles.len());
        for handle in handles {
            let (request, asked, outcome, elapsed_ms) = handle
                .await
                .map_err(|e| Failure::failed(format!("a request task failed: {e}")))?;
            let response = outcome.map_err(|e| Failure {
                exit: Exit::Failed,
                message: format!("request {}: {e}", request.id),
            })?;
            if let Some(dir) = &self.record_dir {
                self.record(dir, &request, &asked, &response, elapsed_ms)?;
            }
            answered.push(Answered {
                request,
                asked,
                response,
                elapsed_ms,
            });
        }
        Ok(answered)
    }

    fn record(
        &self,
        dir: &Path,
        request: &Request,
        asked: &Questions,
        response: &Response,
        elapsed_ms: u64,
    ) -> Result<()> {
        let mut recording = Recording::new(request.id.clone(), response.clone(), elapsed_ms);
        recording.fingerprint = Some(request_fingerprint(&request.state, asked));
        recording.rubric = Some(self.rubric_name.clone());
        recording.server = self.server.clone();
        recording.recorded_at = Some(now_rfc3339());
        let yaml = recording_to_yaml(&recording)?;
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{}.jud", request.id));
        judgment::eval::write_atomically(&path, |f| {
            use std::io::Write;
            f.write_all(yaml.as_bytes())
        })?;
        Ok(())
    }
}

/// A state's size on the wire, for the budget and the log.
pub fn state_bytes(state: &Value) -> usize {
    state.to_string().len()
}
