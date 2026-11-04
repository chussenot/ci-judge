//! Secrets never leave the runner. Every string in a state is passed
//! through these patterns before any network call; a match is replaced
//! by `[REDACTED:<kind>]`, so a question can still see that something
//! was there.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

struct Pattern {
    kind: &'static str,
    regex: Regex,
}

static PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    let p = |kind, re| Pattern {
        kind,
        regex: Regex::new(re).expect("a static pattern compiles"),
    };
    vec![
        p(
            "private-key",
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----",
        ),
        p("aws-access-key", r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b"),
        p(
            "github-token",
            r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36,}\b|\bgithub_pat_[A-Za-z0-9_]{22,}\b",
        ),
        p(
            "gitlab-token",
            r"\bglpat-[A-Za-z0-9_-]{20,}\b|\bglrt-[A-Za-z0-9_-]{20,}\b",
        ),
        p("slack-token", r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b"),
        p("google-api-key", r"\bAIza[0-9A-Za-z_-]{35}\b"),
        p(
            "jwt",
            r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b",
        ),
        p("bearer", r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{16,}"),
        p(
            "basic-auth-url",
            r"(?i)\b[a-z][a-z0-9+.-]*://[^\s/:@]+:[^\s/@]+@",
        ),
        p(
            "assignment",
            r#"(?i)[A-Za-z0-9_.-]*(?:password|passwd|secret|token|api[_-]?key|access[_-]?key|private[_-]?key|client[_-]?secret)[A-Za-z0-9_.-]*\s*[:=]\s*["']?[^\s"',;]{8,}"#,
        ),
    ]
});

/// Redact every string in `value`, in place; keys are left alone.
pub fn value(value: &mut Value) {
    match value {
        Value::String(s) => {
            let redacted = text(s);
            if redacted != *s {
                *s = redacted;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(self::value),
        Value::Object(map) => map.values_mut().for_each(self::value),
        _ => {}
    }
}

/// Redact one text.
pub fn text(text: &str) -> String {
    let mut out = text.to_owned();
    for pattern in PATTERNS.iter() {
        if pattern.regex.is_match(&out) {
            let replacement = format!("[REDACTED:{}]", pattern.kind);
            out = pattern
                .regex
                .replace_all(&out, replacement.as_str())
                .into_owned();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_known_shapes() {
        let t = text(
            "key AKIAABCDEFGHIJKLMNOP and token ghp_abcdefghijklmnopqrstuvwxyz0123456789AB here",
        );
        assert!(t.contains("[REDACTED:aws-access-key]"), "{t}");
        assert!(t.contains("[REDACTED:github-token]"), "{t}");
        assert!(!t.contains("AKIA"), "{t}");
    }

    #[test]
    fn redacts_assignments_and_urls() {
        let t = text("DB_PASSWORD=hunter2hunter2 url=postgres://app:s3cr3tpass@db/x");
        assert!(t.contains("[REDACTED:assignment]"), "{t}");
        assert!(!t.contains("s3cr3tpass"), "{t}");
    }

    #[test]
    fn leaves_ordinary_text() {
        let t =
            "resource \"aws_rds_instance\" \"main\" is deleted; password rotation is documented";
        assert_eq!(text(t), t);
    }

    #[test]
    fn walks_values() {
        let mut v = serde_json::json!({"a": ["glpat-abcdefghijklmnopqrstuv"], "b": {"c": 1}});
        value(&mut v);
        assert_eq!(v["a"][0], "[REDACTED:gitlab-token]");
        assert_eq!(v["b"]["c"], 1);
    }
}
