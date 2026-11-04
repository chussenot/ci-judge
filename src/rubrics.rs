//! The bundled rubric catalog. Blocker and critical severities are allowed
//! only here: a repo-local manifest is capped at `major`, so a team cannot
//! edit the rubric that blocks its own merges.

/// One bundled rubric: its manifest and the files beside it.
pub struct Bundled {
    pub name: &'static str,
    pub files: &'static [(&'static str, &'static str)],
}

macro_rules! bundled {
    ($name:literal, [$($file:literal),+ $(,)?]) => {
        Bundled {
            name: $name,
            files: &[$(($file, include_str!(concat!("../rubrics/", $name, "/", $file)))),+],
        }
    };
}

pub static CATALOG: &[Bundled] = &[
    bundled!(
        "infra-plan",
        ["manifest.yml", "infra-plan.jud", "infra-plan-cases.jud"]
    ),
    bundled!(
        "mr-hygiene",
        ["manifest.yml", "mr-hygiene.jud", "mr-hygiene-cases.jud"]
    ),
    bundled!(
        "failed-job-triage",
        [
            "manifest.yml",
            "failed-job-triage.jud",
            "failed-job-triage-cases.jud"
        ]
    ),
];

pub fn names() -> Vec<&'static str> {
    CATALOG.iter().map(|b| b.name).collect()
}

pub fn manifest(name: &str) -> Option<&'static str> {
    file(name, "manifest.yml")
}

pub fn file(name: &str, file: &str) -> Option<&'static str> {
    CATALOG.iter().find(|b| b.name == name).and_then(|b| {
        b.files
            .iter()
            .find(|(f, _)| *f == file)
            .map(|(_, text)| *text)
    })
}
