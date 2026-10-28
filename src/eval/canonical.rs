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

use crate::asset::Assets;
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
/// same request answered by two models yields two recordings under one
/// name: [`request_fingerprint_with`] for a request without images.
pub fn request_fingerprint(state: &Value, questions: &Questions) -> String {
    request_fingerprint_with(state, questions, &Assets::new())
}

/// The fingerprint of a request with its images: `{"assets": {reference:
/// "sha256:…"}, "questions": …, "state": …}`, each image named by the
/// SHA-256 of its bytes ([`Assets::digests`]) rather than by its base64,
/// so re-encoding changes nothing; without images the `assets` key is left
/// out, so a request without them fingerprints as it always has and every
/// recording made before images stays valid. Two requests that differ only
/// in an image are two fingerprints, which is why the images are in it.
pub fn request_fingerprint_with(state: &Value, questions: &Questions, assets: &Assets) -> String {
    fingerprint(&request_value(state, questions, assets))
}

/// The value both request fingerprints are over ([`request_fingerprint_with`]
/// and [`crate::eval::request_hash_with`]).
pub(crate) fn request_value(state: &Value, questions: &Questions, assets: &Assets) -> Value {
    let questions = serde_json::to_value(questions).unwrap_or(Value::Null);
    let mut object = serde_json::Map::new();
    if !assets.is_empty() {
        object.insert(
            "assets".to_owned(),
            Value::Object(
                assets
                    .digests()
                    .into_iter()
                    .map(|(k, v)| (k, Value::String(v)))
                    .collect(),
            ),
        );
    }
    object.insert("questions".to_owned(), questions);
    object.insert("state".to_owned(), state.clone());
    Value::Object(object)
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

/// A number as RFC 8785 reads it, a double, as ECMAScript's `Number` does:
/// an integer up to 2^53 prints as its own digits, one beyond as the nearest
/// double, so integers that round to one double share a fingerprint.
fn write_number(out: &mut String, n: &serde_json::Number) {
    if let Some(f) = n.as_f64() {
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
    // serde_json's shortest round-trip digits break a tie to the even digit,
    // as ECMAScript does; Rust's `{:e}` rounds it up (RFC 8785 Appendix B).
    let shortest = serde_json::Value::from(number.abs()).to_string();
    let (mantissa, exponent) = shortest.split_once('e').unwrap_or((&shortest, "0"));
    let exponent: i64 = exponent.parse().unwrap_or(0);
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let all = format!("{whole}{fraction}");
    let significant = all.trim_start_matches('0');
    let leading = i64::try_from(all.len() - significant.len()).unwrap_or(0);
    let digits = significant.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let digit_count = i64::try_from(digits.len()).unwrap_or(i64::MAX);
    // ECMAScript's `n`: the value is `digits × 10^(point − digit_count)`.
    let point = i64::try_from(whole.len()).unwrap_or(0) + exponent - leading;
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
            // An integer is exact up to 2^53, then the nearest double, as
            // node and every RFC 8785 implementation print it.
            (json!(9_007_199_254_740_992_u64), "9007199254740992"),
            (json!(-9_007_199_254_740_992_i64), "-9007199254740992"),
            (json!(9_007_199_254_740_993_u64), "9007199254740992"),
            (json!(1_234_567_890_123_456_789_u64), "1234567890123456800"),
            (json!(u64::MAX), "18446744073709552000"),
            (json!(i64::MIN), "-9223372036854776000"),
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

    /// RFC 8785 Appendix B's table: the IEEE 754 bit pattern and the text
    /// the RFC says it prints as (the NaN row is an error there, and is not
    /// a JSON number here either).
    #[test]
    fn rfc_8785_appendix_b_numbers() {
        for (bits, want) in [
            (0x0000_0000_0000_0000_u64, "0"),
            (0x8000_0000_0000_0000, "0"),
            (0x0000_0000_0000_0001, "5e-324"),
            (0x8000_0000_0000_0001, "-5e-324"),
            (0x7fef_ffff_ffff_ffff, "1.7976931348623157e+308"),
            (0xffef_ffff_ffff_ffff, "-1.7976931348623157e+308"),
            (0x4340_0000_0000_0000, "9007199254740992"),
            (0xc340_0000_0000_0000, "-9007199254740992"),
            (0x4430_0000_0000_0000, "295147905179352830000"),
            (0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22"),
            (0x44b5_2d02_c7e1_4af6, "1e+23"),
            (0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23"),
            (0x444b_1ae4_d6e2_ef4e, "999999999999999700000"),
            (0x444b_1ae4_d6e2_ef4f, "999999999999999900000"),
            (0x444b_1ae4_d6e2_ef50, "1e+21"),
            (0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7"),
            (0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
            (0x41b3_de43_5555_5553, "333333333.3333332"),
            (0x41b3_de43_5555_5554, "333333333.33333325"),
            (0x41b3_de43_5555_5555, "333333333.3333333"),
            (0x41b3_de43_5555_5556, "333333333.3333334"),
            (0x41b3_de43_5555_5557, "333333333.33333343"),
            (0xbecb_f647_612f_3696, "-0.0000033333333333333333"),
        ] {
            let f = f64::from_bits(bits);
            assert_eq!(to_string(&json!(f)), want, "{bits:#018x}");
        }
    }

    /// The Appendix B row where two shortest digit strings are equally near
    /// the double (1424953923781206.25): ECMAScript takes the even one, `.2`,
    /// where Rust's `{:e}` would take `.3`.
    #[test]
    fn rfc_8785_appendix_b_shortest_digit_tie_takes_the_even_digit() {
        let f = f64::from_bits(0x4314_3ff3_c1cb_0959);
        assert_eq!(to_string(&json!(f)), "1424953923781206.2");
    }

    /// RFC 8785 §3.2.2.2: `"` and `\` and the controls are escaped (the five
    /// with short forms, the rest as lowercase `\u00xx`), every other
    /// character is written as itself, DEL and the line separators included.
    #[test]
    fn strings_escape_as_rfc_8785_says_for_every_ascii_and_the_edge_scalars() {
        let rule = |c: char| match c {
            '"' => "\\\"".to_owned(),
            '\\' => "\\\\".to_owned(),
            '\u{8}' => "\\b".to_owned(),
            '\t' => "\\t".to_owned(),
            '\n' => "\\n".to_owned(),
            '\u{c}' => "\\f".to_owned(),
            '\r' => "\\r".to_owned(),
            c if (c as u32) < 0x20 => format!("\\u{:04x}", c as u32),
            c => c.to_string(),
        };
        let chars: Vec<char> = (0..128_u8)
            .map(char::from)
            .chain(['\u{2028}', '\u{2029}', '\u{FFFF}'])
            .collect();
        for c in &chars {
            let want = format!("\"{}\"", rule(*c));
            assert_eq!(to_string(&Value::String(c.to_string())), want, "{c:?}");
        }
        let all: String = chars.iter().collect();
        let want = format!("\"{}\"", chars.iter().map(|c| rule(*c)).collect::<String>());
        assert_eq!(to_string(&Value::String(all)), want);
    }

    proptest::proptest! {
        /// Whatever finite double is drawn from its bits, the canonical text
        /// reads back as the same double: nothing is lost in the shortest form.
        #[test]
        fn a_canonical_number_reads_back_as_the_same_double(bits in proptest::num::u64::ANY) {
            let f = f64::from_bits(bits);
            proptest::prop_assume!(f.is_finite());
            let text = to_string(&json!(f));
            let back: f64 = text.parse().unwrap();
            // Same bits, bar the sign of zero, which prints as `0`.
            proptest::prop_assert!(back.to_bits() == f.to_bits() || back == 0.0 && f == 0.0, "{}", text);
        }
    }
}
