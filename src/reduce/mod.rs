//! Reducers turn raw tool output into a JSON value for the state and,
//! where the output is a list of things, into items with locators. A
//! finding points back at an item, so the item must know its path and
//! line.

pub mod diff;
pub mod junit;
pub mod tfplan;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Failure, Result};

/// One thing a per-item question can be asked about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// A short, file-safe key, unique among the source's items.
    pub key: String,
    /// What the question sees under the request's item key.
    pub value: Value,
    /// Where a finding about this item points, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

/// A reduced source: what the global state carries and the items it yields.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Reduced {
    pub value: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<Item>,
}

/// Make a string file-safe: `[A-Za-z0-9._-]`, as a recording name needs.
pub fn safe_key(text: &str) -> String {
    let mut out: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    while out.starts_with(['.', '_', '-']) {
        out.remove(0);
    }
    if out.is_empty() {
        out.push_str("item");
    }
    out.truncate(80);
    out
}

/// Run the named reducer over a text; `None` picks the source's default.
pub fn apply(reducer: &str, text: &str, tail_lines: Option<usize>) -> Result<Reduced> {
    match reducer {
        "tfplan" => tfplan::reduce(text),
        "junit" => junit::reduce(text),
        "diffhunks" => Ok(diff::reduce(text)),
        "json" => {
            let value: Value =
                serde_json::from_str(text).map_err(|e| Failure::usage(format!("not JSON: {e}")))?;
            Ok(Reduced {
                value,
                items: Vec::new(),
            })
        }
        "text" => Ok(Reduced {
            value: Value::String(match tail_lines {
                Some(n) => crate::platform::tail(text, n),
                None => text.to_owned(),
            }),
            items: Vec::new(),
        }),
        "lines" => Ok(Reduced {
            value: Value::Array(text.lines().map(|l| Value::String(l.to_owned())).collect()),
            items: Vec::new(),
        }),
        other => Err(Failure::usage(format!(
            "unknown reducer `{other}` (tfplan, junit, diffhunks, json, text, lines)"
        ))),
    }
}
