//! From collected sources to the requests a rubric is asked: one global
//! request over the whole state, and one request per item for each
//! request shape that takes items.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::manifest::{Manifest, RequestShape};
use crate::reduce::{Item, Reduced};

/// One request: a state and, for an item request, where its findings point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// File-safe and unique within the run: `<slug>-<shape>[-<item>]`.
    pub id: String,
    /// The request shape's name.
    pub shape: String,
    pub state: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

/// What `build` left out.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Truncation {
    /// Items dropped past a shape's cap, per shape.
    pub items_dropped: IndexMap<String, usize>,
    /// State keys dropped to fit the byte budget.
    pub keys_dropped: Vec<String>,
}

impl Truncation {
    pub fn any(&self) -> bool {
        !self.items_dropped.is_empty() || !self.keys_dropped.is_empty()
    }
}

/// A source after collection, with its manifest priority.
#[derive(Debug, Clone)]
pub struct Collected {
    pub key: String,
    pub priority: u32,
    pub reduced: Reduced,
}

fn item_matches(item: &Item, shape: &RequestShape) -> bool {
    shape.filter.iter().all(|(field, allowed)| {
        item.value
            .get(field)
            .map(|v| match v {
                Value::String(s) => allowed.iter().any(|a| a == s),
                other => allowed.iter().any(|a| a == &other.to_string()),
            })
            .unwrap_or(false)
    })
}

/// Build every request of the run.
pub fn build(
    manifest: &Manifest,
    slug: &str,
    sources: &[Collected],
    budget_bytes: usize,
    max_items: Option<usize>,
) -> (Vec<Request>, Truncation) {
    let mut truncation = Truncation::default();
    let mut global: IndexMap<String, (u32, Value)> = IndexMap::new();
    for s in sources {
        global.insert(s.key.clone(), (s.priority, s.reduced.value.clone()));
    }
    // Fit the budget: drop the highest-priority-number keys first, never the item sources' keys
    // (an item request carries only its item) until nothing else is left.
    let item_sources: Vec<&str> = manifest
        .requests
        .iter()
        .filter_map(|r| r.items.as_deref())
        .collect();
    loop {
        let size = Value::Object(
            global
                .iter()
                .map(|(k, (_, v))| (k.clone(), v.clone()))
                .collect(),
        )
        .to_string()
        .len();
        if size <= budget_bytes {
            break;
        }
        let victim = global
            .iter()
            .filter(|(k, _)| !item_sources.contains(&k.as_str()))
            .max_by_key(|(_, (p, _))| *p)
            .map(|(k, _)| k.clone())
            .or_else(|| {
                global
                    .iter()
                    .max_by_key(|(_, (p, _))| *p)
                    .map(|(k, _)| k.clone())
            });
        match victim {
            Some(key) => {
                global.shift_remove(&key);
                truncation.keys_dropped.push(key);
            }
            None => break,
        }
    }

    let mut requests = Vec::new();
    for shape in &manifest.requests {
        match &shape.items {
            None => {
                let mut state: serde_json::Map<String, Value> = global
                    .iter()
                    .map(|(k, (_, v))| (k.clone(), v.clone()))
                    .collect();
                // A global request also carries a key named after its shape, so a
                // rubric asks a question only there with `when: <shape name>`;
                // a state source of that name keeps its own value.
                if !state.contains_key(&shape.name) {
                    let counts: serde_json::Map<String, Value> = sources
                        .iter()
                        .filter(|s| item_sources.contains(&s.key.as_str()))
                        .map(|s| (s.key.clone(), Value::from(s.reduced.items.len())))
                        .collect();
                    state.insert(
                        shape.name.clone(),
                        serde_json::json!({ "request": shape.name, "items": counts }),
                    );
                }
                requests.push(Request {
                    id: format!("{slug}-{}", shape.name),
                    shape: shape.name.clone(),
                    state: Value::Object(state),
                    item_key: None,
                    path: None,
                    line: None,
                });
            }
            Some(source_key) => {
                let Some(source) = sources.iter().find(|s| &s.key == source_key) else {
                    continue;
                };
                let items: Vec<&Item> = source
                    .reduced
                    .items
                    .iter()
                    .filter(|i| item_matches(i, shape))
                    .collect();
                let cap = shape
                    .max_items
                    .into_iter()
                    .chain(max_items)
                    .min()
                    .unwrap_or(usize::MAX);
                if items.len() > cap {
                    truncation
                        .items_dropped
                        .insert(shape.name.clone(), items.len() - cap);
                }
                for item in items.into_iter().take(cap) {
                    // An item request carries no item source at all, so a global
                    // question's `when` never matches it.
                    let mut state: serde_json::Map<String, Value> = global
                        .iter()
                        .filter(|(k, _)| !item_sources.contains(&k.as_str()))
                        .map(|(k, (_, v))| (k.clone(), v.clone()))
                        .collect();
                    state.insert(shape.item_key().to_owned(), item.value.clone());
                    requests.push(Request {
                        id: format!("{slug}-{}-{}", shape.name, item.key),
                        shape: shape.name.clone(),
                        state: Value::Object(state),
                        item_key: Some(item.key.clone()),
                        path: item.path.clone(),
                        line: item.line,
                    });
                }
            }
        }
    }
    (requests, truncation)
}

/// Read `a.b.c` from a state.
pub fn lookup<'a>(state: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = state;
    for seg in path.split('.') {
        cur = match cur {
            Value::Object(m) => m.get(seg)?,
            Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Fill `{a.b}` from a state and `{name}` from `extra`, in a template.
pub fn render(template: &str, state: &Value, extra: &IndexMap<&str, String>) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(rest);
            return out;
        };
        let key = &after[..end];
        let value = extra.get(key).cloned().or_else(|| {
            lookup(state, key).map(|v| match v {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            })
        });
        match value {
            Some(v) => out.push_str(&v),
            None => {
                out.push('{');
                out.push_str(key);
                out.push('}');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_state_paths_and_extras() {
        let state = serde_json::json!({"change": {"action": "delete", "address": "aws_db.main"}});
        let mut extra = IndexMap::new();
        extra.insert("p", "0.91".to_owned());
        assert_eq!(
            render(
                "{change.action} of {change.address} (p={p}) {missing}",
                &state,
                &extra
            ),
            "delete of aws_db.main (p=0.91) {missing}"
        );
    }
}
