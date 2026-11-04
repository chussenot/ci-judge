//! A unified diff to one item per changed file: its hunks, bounded, with
//! the first changed line as the locator.

use serde_json::json;

use super::{Item, Reduced, safe_key};

const MAX_HUNK_CHARS: usize = 4000;

pub fn reduce(text: &str) -> Reduced {
    let mut items = Vec::new();
    let (mut additions, mut deletions) = (0u64, 0u64);
    let mut current: Option<(String, u64, String, u64, u64)> = None; // path, first line, text, +, -
    let flush = |current: &mut Option<(String, u64, String, u64, u64)>, items: &mut Vec<Item>| {
        if let Some((path, line, text, add, del)) = current.take() {
            let truncated = text.len() > MAX_HUNK_CHARS;
            let mut shown = text;
            if truncated {
                let mut end = MAX_HUNK_CHARS;
                while !shown.is_char_boundary(end) {
                    end -= 1;
                }
                shown.truncate(end);
                shown.push_str("\n… (truncated)");
            }
            items.push(Item {
                key: safe_key(&path),
                value: json!({ "path": path, "additions": add, "deletions": del, "truncated": truncated, "hunks": shown }),
                path: Some(path),
                line: Some(line),
            });
        }
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            flush(&mut current, &mut items);
            let path = rest.strip_prefix("b/").unwrap_or(rest).to_owned();
            if path != "/dev/null" {
                current = Some((path, 1, String::new(), 0, 0));
            }
            continue;
        }
        if line.starts_with("--- ") || line.starts_with("diff --git") || line.starts_with("index ")
        {
            continue;
        }
        let Some(cur) = current.as_mut() else {
            continue;
        };
        if let Some(rest) = line.strip_prefix("@@ ") {
            if cur.2.is_empty()
                && let Some(plus) = rest.split_whitespace().nth(1)
                && let Some(start) = plus.trim_start_matches('+').split(',').next()
                && let Ok(n) = start.parse::<u64>()
            {
                cur.1 = n.max(1);
            }
        } else if line.starts_with('+') {
            cur.3 += 1;
            additions += 1;
        } else if line.starts_with('-') {
            cur.4 += 1;
            deletions += 1;
        }
        cur.2.push_str(line);
        cur.2.push('\n');
    }
    flush(&mut current, &mut items);
    let files: Vec<_> = items.iter().map(|i| i.value["path"].clone()).collect();
    Reduced {
        value: json!({ "files": files, "additions": additions, "deletions": deletions,
                       "changed": items.iter().map(|i| i.value.clone()).collect::<Vec<_>>() }),
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_item_per_file_with_first_line() {
        let d = "diff --git a/x.rs b/x.rs\nindex 1..2 100644\n--- a/x.rs\n+++ b/x.rs\n@@ -10,3 +12,4 @@ fn f()\n context\n-old\n+new\n+more\ndiff --git a/y.md b/y.md\n--- /dev/null\n+++ b/y.md\n@@ -0,0 +1 @@\n+hello\n";
        let r = reduce(d);
        assert_eq!(r.items.len(), 2);
        assert_eq!(r.items[0].path.as_deref(), Some("x.rs"));
        assert_eq!(r.items[0].line, Some(12));
        assert_eq!(r.value["additions"], 3);
        assert_eq!(r.value["deletions"], 1);
    }
}
