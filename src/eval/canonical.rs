//! Canonical JSON and the fingerprint over it: the one way two tools name
//! the same request, rubric or document.
//!
//! Byte-hashing fails as soon as two clients order keys differently or print
//! `1.0` and `1` apart, so the hash is over the
//! [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) rendering; `docs/reference/jud-format.md`
//! states the rules and a known vector. The older [`crate::eval::fingerprint`]
//! (FNV-1a over a near-canonical form) still names the committed recordings;
//! a new tool should use this module.

use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::question::Questions;

/// The canonical rendering of `value` per RFC 8785.
pub fn to_string(value: &Value) -> String {
    let mut out = String::new();
    write(&mut out, value);
    out
}

/// `sha256:` and the lowercase hex SHA-256 of the canonical rendering.
pub fn fingerprint(value: &Value) -> String {
    let digest = Sha256::digest(to_string(value).as_bytes());
    let mut hex = String::with_capacity(7 + 64);
    hex.push_str("sha256:");
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The fingerprint of `{"questions": …, "state": …}`, model left out so the
/// same request answered by two models yields two recordings under one name.
pub fn request_fingerprint(state: &Value, questions: &Questions) -> String {
    let questions = serde_json::to_value(questions).unwrap_or(Value::Null);
    fingerprint(&Value::Object(
        [
            ("questions".to_owned(), questions),
            ("state".to_owned(), state.clone()),
        ]
        .into_iter()
        .collect(),
    ))
}

fn write(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => write_number(out, n),
        // serde_json escapes `"`, `\` and the control characters (`\b \f \n
        // \r \t`, else lowercase `\u00xx`) and nothing else: RFC 8785's rule.
        Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap_or_default()),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).unwrap_or_default());
                out.push(':');
                write(out, &map[*key]);
            }
            out.push('}');
        }
    }
}

fn write_number(out: &mut String, n: &serde_json::Number) {
    if let Some(i) = n.as_i64() {
        out.push_str(&i.to_string());
    } else if let Some(u) = n.as_u64() {
        out.push_str(&u.to_string());
    } else if let Some(f) = n.as_f64() {
        out.push_str(&ecmascript(f));
    }
}

/// ECMAScript's `Number.prototype.toString` for a finite `f64`: shortest round-trip digits,
/// positional while the decimal point `n` is in `-6 < n <= 21`, else exponent with a sign.
fn ecmascript(number: f64) -> String {
    if number == 0.0 {
        return "0".to_owned();
    }
    let negative = number < 0.0;
    // Rust's `{:e}` gives the shortest round-trip digits as `d.ddde±x`.
    let sci = format!("{:e}", number.abs());
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exponent: i64 = exponent.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let digit_count = i64::try_from(digits.len()).unwrap_or(i64::MAX);
    // ECMAScript's `n`: the value is `digits × 10^(point − digit_count)`.
    let point = exponent + 1;
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if digit_count <= point && point <= 21 {
        out.push_str(digits);
        for _ in 0..(point - digit_count) {
            out.push('0');
        }
    } else if 0 < point && point <= 21 {
        let split = usize::try_from(point).unwrap_or(0);
        out.push_str(&digits[..split]);
        out.push('.');
        out.push_str(&digits[split..]);
    } else if -6 < point && point <= 0 {
        out.push_str("0.");
        for _ in 0..(-point) {
            out.push('0');
        }
        out.push_str(digits);
    } else {
        let exp = point - 1;
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        out.push_str(&exp.abs().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use serde_json::json;

    #[test]
    fn keys_are_sorted_and_whitespace_dropped() {
        let v = json!({"b": [1, 2], "a": {"z": null, "y": true}});
        assert_eq!(to_string(&v), r#"{"a":{"y":true,"z":null},"b":[1,2]}"#);
    }

    #[test]
    fn keys_sort_by_utf16_units_as_rfc_8785_requires() {
        // U+1D11E (surrogate pair, first unit 0xD834) sorts before U+FB01
        // in UTF-16 and after it by scalar value.
        let v = json!({"\u{FB01}": 1, "\u{1D11E}": 2, "a": 3});
        assert_eq!(to_string(&v), "{\"a\":3,\"\u{1D11E}\":2,\"\u{FB01}\":1}");
    }

    #[test]
    fn numbers_print_as_ecmascript_does() {
        for (input, want) in [
            (json!(1), "1"),
            (json!(1.0), "1"),
            (json!(-0.0), "0"),
            (json!(0.1), "0.1"),
            (json!(123.456), "123.456"),
            (json!(1e21), "1e+21"),
            (json!(1e20), "100000000000000000000"),
            (json!(1e-7), "1e-7"),
            (json!(0.000_001), "0.000001"),
            (json!(1.5e-9), "1.5e-9"),
            (json!(-2.5), "-2.5"),
            (json!(u64::MAX), "18446744073709551615"),
        ] {
            assert_eq!(to_string(&input), want, "{input}");
        }
    }

    #[test]
    fn strings_escape_only_what_json_requires() {
        let v = json!("é \"q\" \\ \n \u{1} /");
        assert_eq!(to_string(&v), r#""é \"q\" \\ \n \u0001 /""#);
    }

    #[test]
    fn the_fingerprint_is_the_same_whatever_the_key_order_or_number_spelling() {
        let a = json!({"state": {"x": 1.0, "y": "t"}, "questions": {}});
        let b: Value =
            serde_json::from_str(r#"{"questions": {}, "state": {"y": "t", "x": 1}}"#).unwrap();
        assert_eq!(fingerprint(&a), fingerprint(&b));
        assert!(fingerprint(&a).starts_with("sha256:"));
        assert_eq!(fingerprint(&a).len(), 7 + 64);
    }

    #[test]
    fn a_known_vector() {
        // The vector `docs/reference/jud-format.md` publishes: `printf '{"a":1,"b":"x"}' | sha256sum`.
        let v = json!({"b": "x", "a": 1});
        assert_eq!(to_string(&v), r#"{"a":1,"b":"x"}"#);
        assert_eq!(
            fingerprint(&v),
            "sha256:ecf9e98ec0641e23113ff3ce8bdc78d0ddd249886517fd4a7f68cc83d4e65667"
        );
    }

    #[test]
    fn a_request_fingerprint_ignores_the_model_and_key_order() {
        let mut q = Questions::new();
        q.noul("n", "?", None).unwrap();
        let s1 = json!({"a": 1, "b": 2});
        let s2: Value = serde_json::from_str(r#"{"b": 2, "a": 1}"#).unwrap();
        assert_eq!(request_fingerprint(&s1, &q), request_fingerprint(&s2, &q));
    }
}
