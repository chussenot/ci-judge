//! `ci-judge`: the judgment layer of a CI judgment gate.
//!
//! Three layers make a gate. The [`judgment`] crate answers typed questions
//! about a JSON state with calibrated probabilities and reads them through a
//! `.jud` rubric's policy. A gate (GitLab's `judge-gate` block, or
//! [`gate`] here for GitHub workflows and anywhere else) turns a verdict
//! file into a pipeline decision. This crate is the part in between: it
//! collects the evidence of a merge request, shapes it into one request
//! per evidence item plus one global request, asks the rubric's questions,
//! and writes the verdict file the gate reads.
//!
//! Nothing here generates text. A finding is a template filled from the
//! item a question was asked about and the probability it got.

pub mod assess;
pub mod cases;
pub mod collect;
pub mod config;
pub mod error;
pub mod eval;
pub mod gate;
pub mod manifest;
pub mod platform;
pub mod redact;
pub mod reduce;
pub mod requests;
pub mod rubrics;
pub mod run;
pub mod verdict;

pub use error::{Exit, Failure};
