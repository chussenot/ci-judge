//! JUnit XML to failing test cases, one item each.

use serde_json::json;

use super::{Item, Reduced, safe_key};
use crate::error::{Failure, Result};

pub fn reduce(text: &str) -> Result<Reduced> {
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| Failure::usage(format!("junit: not XML: {e}")))?;
    let mut items = Vec::new();
    let (mut total, mut failed, mut skipped) = (0u64, 0u64, 0u64);
    for case in doc.descendants().filter(|n| n.has_tag_name("testcase")) {
        total += 1;
        let name = case.attribute("name").unwrap_or("").to_owned();
        let classname = case.attribute("classname").unwrap_or("").to_owned();
        let file = case.attribute("file").map(str::to_owned);
        let line = case.attribute("line").and_then(|l| l.parse::<u64>().ok());
        let mut failure = None;
        for child in case.children().filter(roxmltree::Node::is_element) {
            match child.tag_name().name() {
                "failure" | "error" => {
                    failure = Some(json!({
                        "kind": child.tag_name().name(),
                        "message": child.attribute("message").unwrap_or(""),
                        "type": child.attribute("type").unwrap_or(""),
                        "text": crate::platform::tail(child.text().unwrap_or("").trim(), 60),
                    }));
                }
                "skipped" => skipped += 1,
                _ => {}
            }
        }
        let Some(failure) = failure else { continue };
        failed += 1;
        let suite = case
            .ancestors()
            .find(|a| a.has_tag_name("testsuite"))
            .and_then(|s| s.attribute("name"))
            .unwrap_or("")
            .to_owned();
        let full = if classname.is_empty() {
            name.clone()
        } else {
            format!("{classname}.{name}")
        };
        items.push(Item {
            key: safe_key(&full),
            value: json!({
                "name": name,
                "classname": classname,
                "suite": suite,
                "file": file,
                "line": line,
                "failure": failure,
            }),
            path: file,
            line,
        });
    }
    Ok(Reduced {
        value: json!({ "total": total, "failed": failed, "skipped": skipped,
                       "failing": items.iter().map(|i| i.value.clone()).collect::<Vec<_>>() }),
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_item_per_failing_case() {
        let xml = r#"<testsuites><testsuite name="unit"><testcase classname="a.B" name="ok"/>
          <testcase classname="a.B" name="boom" file="src/b.rs" line="12"><failure message="assert">stack</failure></testcase>
          <testcase name="skip"><skipped/></testcase></testsuite></testsuites>"#;
        let r = reduce(xml).unwrap();
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.items[0].path.as_deref(), Some("src/b.rs"));
        assert_eq!(r.items[0].line, Some(12));
        assert_eq!(r.value["total"], 3);
        assert_eq!(r.value["skipped"], 1);
    }
}
