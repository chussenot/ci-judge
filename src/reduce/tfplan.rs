//! `terraform show -json tfplan` to resource changes. The plan JSON has no
//! file positions, so each change is located by scanning the `.tf` files
//! of the working directory for its `resource` block.

use std::path::Path;

use serde_json::{Value, json};

use super::{Item, Reduced, safe_key};
use crate::error::{Failure, Result};

/// The action a change takes, in the vocabulary a question reads.
fn action(actions: &[Value]) -> &'static str {
    let a: Vec<&str> = actions.iter().filter_map(Value::as_str).collect();
    match a.as_slice() {
        ["create"] => "create",
        ["delete"] => "delete",
        ["update"] => "update",
        ["read"] => "read",
        ["no-op"] => "no-op",
        ["delete", "create"] | ["create", "delete"] => "replace",
        _ => "other",
    }
}

pub fn reduce(text: &str) -> Result<Reduced> {
    let plan: Value =
        serde_json::from_str(text).map_err(|e| Failure::usage(format!("tfplan: not JSON: {e}")))?;
    let changes = plan
        .get("resource_changes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    let mut summary = serde_json::Map::new();
    for change in &changes {
        let address = change
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let actions = change
            .pointer("/change/actions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let action = action(&actions);
        *summary.entry(action).or_insert(json!(0)) =
            json!(summary.get(action).and_then(Value::as_u64).unwrap_or(0) + 1);
        if matches!(action, "no-op" | "read") {
            continue;
        }
        let r#type = change
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let name = change
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let located = locate(Path::new("."), &r#type, &name);
        let value = json!({
            "address": address,
            "type": r#type,
            "name": name,
            "action": action,
            "module": change.get("module_address").cloned().unwrap_or(Value::Null),
            "provider": change.get("provider_name").cloned().unwrap_or(Value::Null),
            "replace_paths": change.pointer("/change/replace_paths").cloned().unwrap_or(Value::Null),
            "action_reason": change.get("action_reason").cloned().unwrap_or(Value::Null),
            "before_keys": keys(change.pointer("/change/before")),
            "after_keys": keys(change.pointer("/change/after")),
            "path": located.as_ref().map(|(p, _)| Value::String(p.clone())).unwrap_or(Value::Null),
        });
        items.push(Item {
            key: safe_key(&address),
            value,
            path: located.as_ref().map(|(p, _)| p.clone()),
            line: located.map(|(_, l)| l),
        });
    }
    let value = json!({
        "format_version": plan.get("format_version").cloned().unwrap_or(Value::Null),
        "terraform_version": plan.get("terraform_version").cloned().unwrap_or(Value::Null),
        "summary": Value::Object(summary),
        "changes": items.iter().map(|i| i.value.clone()).collect::<Vec<_>>(),
    });
    Ok(Reduced { value, items })
}

/// The top-level attribute names of a resource's before or after, so a
/// question can see what the resource is without its values (which can
/// be sensitive).
fn keys(value: Option<&Value>) -> Value {
    match value.and_then(Value::as_object) {
        Some(map) => Value::Array(map.keys().map(|k| Value::String(k.clone())).collect()),
        None => Value::Null,
    }
}

/// Find `resource "<type>" "<name>"` in the `.tf` files under `root`.
pub fn locate(root: &Path, r#type: &str, name: &str) -> Option<(String, u64)> {
    let needle = format!("resource \"{type}\" \"{name}\"");
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.path());
        for entry in entries {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if !matches!(
                    file_name.as_str(),
                    ".git" | ".terraform" | "node_modules" | "target"
                ) {
                    stack.push(path);
                }
                continue;
            }
            if !file_name.ends_with(".tf") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (i, line) in text.lines().enumerate() {
                if line.trim_start().starts_with(&needle) {
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    return Some((rel.trim_start_matches("./").to_owned(), i as u64 + 1));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduces_changes_to_items() {
        let plan = r#"{"format_version":"1.2","resource_changes":[
          {"address":"aws_db_instance.main","type":"aws_db_instance","name":"main","change":{"actions":["delete"],"before":{"id":"x","password":"s"},"after":null}},
          {"address":"aws_security_group.app","type":"aws_security_group","name":"app","change":{"actions":["delete","create"],"before":{},"after":{}}},
          {"address":"aws_s3_bucket.logs","type":"aws_s3_bucket","name":"logs","change":{"actions":["no-op"]}}
        ]}"#;
        let r = reduce(plan).unwrap();
        assert_eq!(r.items.len(), 2);
        assert_eq!(r.items[0].value["action"], "delete");
        assert_eq!(r.items[0].key, "aws_db_instance.main");
        assert_eq!(r.items[1].value["action"], "replace");
        assert_eq!(r.value["summary"]["no-op"], 1);
        // Values never reach the state, only attribute names.
        assert!(
            r.items[0].value["before_keys"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k == "password")
        );
        assert!(r.items[0].value.get("before").is_none());
    }
}
