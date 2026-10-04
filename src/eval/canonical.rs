//! Canonical JSON and the fingerprint over it: the one way two tools name
//! the same request, rubric or document.
//!
//! A recording is found by the request it answers, and a policy is tuned
//! against a particular set of questions, so both need a name for "this
//! JSON" that does not depend on who serialised it. Byte-hashing the request
//! fails as soon as two clients order keys differently or print `1.0` and
//! `1` apart; the fix is to hash a canonical rendering. The rendering here is
//! [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785), the JSON
//! Canonicalization Scheme: no whitespace, object keys sorted by their UTF-16
//! code units, strings escaped as JSON requires and no more, and numbers
//! written as ECMAScript's `Number.prototype.toString` writes them, so `1.0`
//! and `1` are one number and `1e21` is `1e+21`. Any implementation of the
//! scheme, in any language, produces the same bytes and so the same hash.
//!
//! The fingerprint is `sha256:` followed by the lowercase hex SHA-256 of
//! those bytes. [`request_fingerprint`] names a request by its `state` and
//! `questions` only: the model is not part of the key, so the same request
//! answered by two models yields two recordings under one name, which is
//! what a comparison needs.
//!
//! The older [`crate::eval::fingerprint`] and [`crate::eval::request_hash`]
//! are a 64-bit FNV-1a over a near-canonical form (keys sorted by Unicode
//! scalar value, numbers as `serde_json` prints them). They still name the
//! committed recordings and stay as they are; a new tool should use this
//! module.

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

/// The fingerprint of a request: `{"questions": …, "state": …}`, model
/// excluded, so the same state and questions name the same request whatever
/// model, alias or client asked.
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
        // serde_json escapes `"`, `\` and the control characters, the
        // latter as `\b \f \n \r \t` or lowercase `\u00xx`, and nothing
        // else: the escaping RFC 8785 requires.
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

/// A number as ECMAScript prints it: an integer without a fraction, a
/// fraction in positional notation between `1e-7` and `1e21`, exponent
/// notation with an explicit sign outside that range, the shortest digits
/// that round-trip throughout.
fn write_number(out: &mut String, n: &serde_json::Number) {
    if let Some(i) = n.as_i64() {
        out.push_str(&i.to_string());
    } else if let Some(u) = n.as_u64() {
        out.push_str(&u.to_string());
    } else if let Some(f) = n.as_f64() {
        out.push_str(&ecmascript(f));
    }
}

/// `Number.prototype.toString` for a finite `f64`.
fn ecmascript(number: f64) -> String {
    if number == 0.0 {
        return "0".to_owned();
    }
    let negative = number < 0.0;
    // Rust's `{:e}` is the shortest round-trip digits, as `d.ddde±x`.
    let sci = format!("{:e}", number.abs());
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exponent: i64 = exponent.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let digit_count = i64::try_from(digits.len()).unwrap_or(i64::MAX);
    // The position of the decimal point relative to the digits (ECMAScript's
    // `n`): `digits × 10^(point − digit_count)`.
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
        // U+1D11E (a surrogate pair in UTF-16, first unit 0xD834) sorts
        // before U+FB01 (one unit) in UTF-16 and after it by scalar value.
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
        // Recomputable with any SHA-256 tool over the bytes `{"a":1,"b":"x"}`
        // (`printf '{"a":1,"b":"x"}' | sha256sum`).
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
