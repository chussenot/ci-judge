//! Shadow mode grows a `Cases` document: one unlabelled case per request,
//! so a reviewer fills `expect` and the recordings become a golden set.

use std::path::Path;

use judgment::jud::{Case, Cases, Document, parse};

use crate::error::{Failure, Result};
use crate::requests::Request;

/// Append the run's requests as cases to `path`, creating the document
/// when it does not exist; a case whose id is already there is left as is.
pub fn append(path: &Path, rubric_name: &str, requests: &[Request]) -> Result<usize> {
    let mut cases = match std::fs::read_to_string(path) {
        Ok(text) => match parse(&text)? {
            Document::Cases(cases) => cases,
            _ => {
                return Err(Failure::usage(format!(
                    "{} is not a Cases document",
                    path.display()
                )));
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Cases {
            name: format!("{rubric_name}-cases"),
            description: Some(
                "Cases captured by ci-judge in shadow mode; fill `expect` to label them."
                    .to_owned(),
            ),
            rubric: Some(rubric_name.to_owned()),
            ..Cases::default()
        },
        Err(e) => {
            return Err(Failure::usage(format!(
                "cannot read {}: {e}",
                path.display()
            )));
        }
    };
    let mut added = 0;
    for request in requests {
        if cases
            .cases
            .iter()
            .any(|c| c.id.as_deref() == Some(request.id.as_str()))
        {
            continue;
        }
        let mut case = Case::new(request.state.clone());
        case.id = Some(request.id.clone());
        case.tags = vec![request.shape.clone(), "unlabelled".to_owned()];
        case.note = request.path.as_ref().map(|p| match request.line {
            Some(l) => format!("{p}:{l}"),
            None => p.clone(),
        });
        cases.cases.push(case);
        added += 1;
    }
    if added > 0 {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, cases.to_yaml()?)?;
    }
    Ok(added)
}
