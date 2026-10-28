//! Properties of the crate's invariants over generated inputs, where the
//! unit tests check the inputs someone thought of.
//!
//! - A [`Probability`] and a [`Confidence`] hold exactly the values in
//!   `[0, 1]`, on construction and when decoded, and `NaN`, the infinities
//!   and a negative zero are handled as the module docs say.
//! - The canonical fingerprint does not depend on the order an object's keys
//!   were written in, names the same value as its canonical text does, and
//!   changes when any leaf does.
//! - [`Response::verify`] accepts a response that fits its questions and
//!   refuses each way of not fitting with the error that names it, the
//!   first in wire order; after it, every handle from the same questions
//!   reads through [`Response::get`].
//! - Decoding a response is total, keeps every probability and confidence
//!   in `[0, 1]`, takes the kind from the exact `type` sent, and writes back
//!   to what it read.
//! - [`Questions`] holds its limits at their edges: 255 options, 2 to 10
//!   levels, no empty and no repeated id.
//!
//! Each property runs 256 cases by default and finishes well inside two
//! seconds; `PROPTEST_CASES=10000` runs more. A failure is shrunk and saved
//! under `proptest-regressions/`, replayed first from then on: commit the
//! file with the fix.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use std::collections::BTreeMap;

use judgment::eval::canonical;
use judgment::{
    Answer, BoundingBox, BoxEdges, Choice, Confidence, Error, Noul, Options, Probability,
    Questions, Response, Score, Usage, options,
};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use serde_json::{Value, json};

/// The default settings, with failures saved under `proptest-regressions/`
/// at the crate root: proptest's own default looks for a `lib.rs` or
/// `main.rs` beside the test file, which an integration test has none of, and
/// would drop the failure it was asked to keep.
fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "proptest-regressions/properties.txt",
        ))),
        // `two_boxes_that_do_not_touch_have_no_overlap` rejects about half its
        // cases; the default 1024 stops it near 2000 under `PROPTEST_CASES`.
        max_global_rejects: 1 << 16,
        ..ProptestConfig::default()
    }
}

// ---------------------------------------------------------------------------
// Probability and Confidence
// ---------------------------------------------------------------------------

/// Floats from both sides of both edges, every class, and the awkward ones
/// by name.
fn any_float() -> impl Strategy<Value = f64> {
    prop_oneof![
        3 => -0.5f64..1.5,
        2 => proptest::num::f64::ANY,
        1 => prop_oneof![
            Just(f64::NAN),
            Just(f64::INFINITY),
            Just(f64::NEG_INFINITY),
            Just(-0.0),
            Just(0.0),
            Just(1.0),
            Just(1.0 + f64::EPSILON),
            Just(1.0 - f64::EPSILON),
            Just(-f64::MIN_POSITIVE),
            Just(f64::MIN_POSITIVE),
        ],
    ]
}

/// The oracle, written apart from the implementation: closed at both ends,
/// false for `NaN`. Two comparisons, where the code under test asks a range.
#[allow(clippy::manual_range_contains)]
fn in_unit_interval(x: f64) -> bool {
    0.0 <= x && x <= 1.0
}

macro_rules! unit_interval {
    ($module:ident, $ty:ident) => {
        mod $module {
            use super::*;

            proptest! {
#![proptest_config(config())]
                #[test]
                fn construction_accepts_exactly_zero_to_one(x in any_float()) {
                    match $ty::new(x) {
                        Ok(value) => {
                            prop_assert!(in_unit_interval(x), "{x} accepted");
                            prop_assert_eq!(value.value().to_bits(), x.to_bits());
                        }
                        Err(Error::NotAProbability { value }) => {
                            prop_assert!(!in_unit_interval(x), "{x} refused");
                            prop_assert_eq!(value.to_bits(), x.to_bits());
                        }
                        Err(other) => prop_assert!(false, "wrong error: {other:?}"),
                    }
                }

                #[test]
                fn decoding_accepts_exactly_zero_to_one_and_round_trips(x in any_float()) {
                    // Only finite numbers reach the wire as JSON numbers.
                    prop_assume!(x.is_finite());
                    let text = serde_json::to_string(&x).unwrap();
                    let decoded = serde_json::from_str::<$ty>(&text);
                    prop_assert_eq!(decoded.is_ok(), in_unit_interval(x), "{}", text);
                    if let Ok(value) = decoded {
                        prop_assert_eq!(value.value().to_bits(), x.to_bits());
                        prop_assert_eq!(serde_json::to_string(&value).unwrap(), text);
                    }
                }

                #[test]
                fn a_threshold_reads_a_value_at_or_above_it(x in 0.0f64..=1.0, t in 0.0f64..=1.0) {
                    prop_assert_eq!($ty::new(x).unwrap().at_least(t), x >= t);
                }
            }

            // A negative zero is equal to zero, which is the point.
            #[allow(clippy::float_cmp)]
            #[test]
            fn the_edges_and_a_negative_zero_are_inside() {
                for edge in [0.0, 1.0, -0.0] {
                    assert!($ty::new(edge).is_ok(), "{edge}");
                }
                assert_eq!($ty::new(-0.0).unwrap().value(), 0.0);
            }

            #[test]
            fn nan_and_the_infinities_are_refused() {
                for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    assert!(
                        matches!($ty::new(bad), Err(Error::NotAProbability { .. })),
                        "{bad}"
                    );
                }
            }

            #[test]
            fn what_is_not_a_json_number_in_range_never_decodes() {
                for text in [
                    "null",
                    "\"0.5\"",
                    "true",
                    "[0.5]",
                    "{}",
                    "NaN",
                    "Infinity",
                    "-Infinity",
                    "1e999",
                    "-0.0000001",
                    "1.0000001",
                    "2",
                ] {
                    assert!(serde_json::from_str::<$ty>(text).is_err(), "{text}");
                }
                for text in ["0", "1", "0.0", "1.0", "-0.0", "0.5", "1e-300"] {
                    assert!(serde_json::from_str::<$ty>(text).is_ok(), "{text}");
                }
            }
        }
    };
}

unit_interval!(probability, Probability);
unit_interval!(confidence, Confidence);

// ---------------------------------------------------------------------------
// The canonical fingerprint
// ---------------------------------------------------------------------------

/// A JSON tree that keeps its keys in a list, so the order they are written
/// in is ours to choose: `serde_json::Value` sorts them in this build.
#[derive(Debug, Clone)]
enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

/// Keys that sort differently by UTF-16 code unit and by UTF-8 byte: a
/// character above U+FFFF (a surrogate pair in UTF-16) against ones in
/// U+E000 to U+FFFF.
fn key() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[ab\u{E000}\u{FF61}\u{10000}\u{1F600}]{0,3}").unwrap()
}

/// A tree to depth four; with `floats` false, one the oracle can render.
fn json(floats: bool) -> impl Strategy<Value = Json> {
    let leaf = prop_oneof![
        Just(Json::Null),
        any::<bool>().prop_map(Json::Bool),
        // Within 2^53, so adding one stays exact.
        (-(1i64 << 53)..(1i64 << 53)).prop_map(Json::Int),
        // A float leaf is a number the canonical form has to format; the
        // oracle does not, so a tree for it has none.
        (-1e9f64..1e9).prop_map(move |f| if floats { Json::Float(f) } else { Json::Int(0) }),
        any::<String>().prop_map(Json::Str),
    ];
    leaf.prop_recursive(4, 32, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Json::Arr),
            prop::collection::btree_map(key(), inner, 0..4)
                .prop_map(|fields| Json::Obj(fields.into_iter().collect())),
        ]
    })
}

/// `tree` as JSON text, each object's keys rotated by `rotate` and reversed
/// when `reverse`: the same value written in another order.
fn render(tree: &Json, rotate: usize, reverse: bool) -> String {
    let text = |s: &str| serde_json::to_string(s).unwrap();
    match tree {
        Json::Null => "null".to_owned(),
        Json::Bool(b) => b.to_string(),
        Json::Int(i) => i.to_string(),
        Json::Float(f) => serde_json::to_string(f).unwrap(),
        Json::Str(s) => text(s),
        Json::Arr(items) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| render(item, rotate, reverse))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Json::Obj(fields) => {
            let mut order: Vec<usize> = (0..fields.len()).collect();
            let count = order.len();
            if count > 0 {
                order.rotate_left(rotate % count);
            }
            if reverse {
                order.reverse();
            }
            format!(
                "{{{}}}",
                order
                    .iter()
                    .map(|&i| format!(
                        "{}:{}",
                        text(&fields[i].0),
                        render(&fields[i].1, rotate, reverse)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

/// How many leaves `tree` has.
fn leaves(tree: &Json) -> usize {
    match tree {
        Json::Arr(items) => items.iter().map(leaves).sum(),
        Json::Obj(fields) => fields.iter().map(|(_, v)| leaves(v)).sum(),
        _ => 1,
    }
}

/// Change the `n`th leaf (depth first) to a different value.
fn change_leaf(tree: &mut Json, n: &mut usize) -> bool {
    match tree {
        Json::Arr(items) => items.iter_mut().any(|item| change_leaf(item, n)),
        Json::Obj(fields) => fields.iter_mut().any(|(_, v)| change_leaf(v, n)),
        leaf => {
            if *n > 0 {
                *n -= 1;
                return false;
            }
            *leaf = match leaf {
                Json::Null => Json::Bool(false),
                Json::Bool(b) => Json::Bool(!*b),
                Json::Int(i) => Json::Int(*i + 1),
                Json::Float(f) => Json::Float(*f + 1.0),
                Json::Str(s) => Json::Str(format!("{s}x")),
                Json::Arr(_) | Json::Obj(_) => unreachable!("not a leaf"),
            };
            true
        }
    }
}

/// The canonical text of a tree without floats, worked out apart from
/// `canonical`: keys by UTF-16 code unit, integers in decimal.
fn oracle(tree: &Json) -> Option<String> {
    Some(match tree {
        Json::Null => "null".to_owned(),
        Json::Bool(b) => b.to_string(),
        Json::Int(i) => i.to_string(),
        Json::Float(_) => return None,
        Json::Str(s) => serde_json::to_string(s).unwrap(),
        Json::Arr(items) => format!(
            "[{}]",
            items
                .iter()
                .map(oracle)
                .collect::<Option<Vec<_>>>()?
                .join(",")
        ),
        Json::Obj(fields) => {
            let mut sorted: Vec<&(String, Json)> = fields.iter().collect();
            sorted.sort_by_key(|(k, _)| k.encode_utf16().collect::<Vec<u16>>());
            format!(
                "{{{}}}",
                sorted
                    .iter()
                    .map(|(k, v)| Some(format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap(),
                        oracle(v)?
                    )))
                    .collect::<Option<Vec<_>>>()?
                    .join(",")
            )
        }
    })
}

proptest! {
#![proptest_config(config())]
    #[test]
    fn the_fingerprint_does_not_depend_on_key_order(
        tree in json(true),
        rotate in 0usize..4,
        reverse in any::<bool>(),
    ) {
        let written = parse(&render(&tree, 0, false));
        let reordered = parse(&render(&tree, rotate, reverse));
        prop_assert_eq!(canonical::to_string(&written), canonical::to_string(&reordered));
        prop_assert_eq!(canonical::fingerprint(&written), canonical::fingerprint(&reordered));
    }

    #[test]
    fn the_canonical_text_is_the_same_value_and_is_its_own_canonical_form(tree in json(true)) {
        let value = parse(&render(&tree, 0, false));
        let text = canonical::to_string(&value);
        prop_assert_eq!(&parse(&text), &value, "canonical text is another value");
        prop_assert_eq!(canonical::to_string(&parse(&text)), text);
    }

    #[test]
    fn keys_sort_by_utf16_code_unit_and_integers_print_in_decimal(tree in json(false)) {
        let value = parse(&render(&tree, 0, false));
        prop_assert_eq!(canonical::to_string(&value), oracle(&tree).unwrap());
    }

    #[test]
    fn changing_any_leaf_changes_the_fingerprint(tree in json(true), which in any::<usize>()) {
        // The `null` guarantees a leaf: an empty array or object has none.
        let tree = Json::Arr(vec![Json::Null, tree]);
        let count = leaves(&tree);
        let mut changed = tree.clone();
        let mut n = which % count;
        prop_assert!(change_leaf(&mut changed, &mut n), "a leaf was not found");
        let before = parse(&render(&tree, 0, false));
        let after = parse(&render(&changed, 0, false));
        prop_assert_ne!(canonical::to_string(&before), canonical::to_string(&after));
        prop_assert_ne!(canonical::fingerprint(&before), canonical::fingerprint(&after));
    }
}

// ---------------------------------------------------------------------------
// Response::verify
// ---------------------------------------------------------------------------

/// A name no generated option, level or id can equal: it holds a NUL.
const BOGUS: &str = "\u{0}not-offered";

/// A question and the answer that fits it, with everything the answer needs
/// to be chosen: what the property then breaks one way at a time.
#[derive(Debug, Clone)]
enum Spec {
    Noul {
        p: f64,
    },
    Choice {
        options: usize,
        pick: usize,
        p: f64,
        confidence: f64,
    },
    Score {
        levels: Vec<Value>,
        echo_as_text: Vec<bool>,
        at: f64,
        p: f64,
        confidence: f64,
    },
    Box {
        top: f64,
        left: f64,
    },
}

fn level() -> impl Strategy<Value = Value> {
    prop_oneof![
        "[a-z ]{1,8}".prop_map(Value::from),
        (0..100).prop_map(|n| json!({ "rank": n })),
    ]
}

fn unit() -> impl Strategy<Value = f64> {
    0.0f64..=1.0
}

fn choice_spec() -> impl Strategy<Value = Spec> {
    (2usize..=6, 0usize..6, unit(), unit()).prop_map(|(options, pick, p, confidence)| {
        Spec::Choice {
            options,
            pick: pick % options,
            p,
            confidence,
        }
    })
}

fn score_spec() -> impl Strategy<Value = Spec> {
    // Up to the limit of ten, so `"10"` and `"2"` sort apart as text.
    prop::collection::vec((level(), any::<bool>()), 2..=10)
        .prop_flat_map(|levels| (Just(levels), unit(), unit(), unit()))
        .prop_map(|(levels, at, p, confidence)| Spec::Score {
            echo_as_text: levels.iter().map(|(_, text)| *text).collect(),
            levels: levels.into_iter().map(|(level, _)| level).collect(),
            at,
            p,
            confidence,
        })
}

fn any_spec() -> impl Strategy<Value = Spec> {
    prop_oneof![
        unit().prop_map(|p| Spec::Noul { p }),
        choice_spec(),
        score_spec(),
        (0.0f64..0.4, 0.0f64..0.4).prop_map(|(top, left)| Spec::Box { top, left }),
    ]
}

/// Some specs with `required` among them, and where it is.
fn specs_with(required: impl Strategy<Value = Spec>) -> impl Strategy<Value = (Vec<Spec>, usize)> {
    (
        prop::collection::vec(any_spec(), 0..3),
        required,
        prop::collection::vec(any_spec(), 0..3),
    )
        .prop_map(|(mut specs, required, after)| {
            let at = specs.len();
            specs.push(required);
            specs.extend(after);
            (specs, at)
        })
}

fn id(i: usize) -> String {
    format!("q{i}")
}

fn option(k: usize) -> String {
    format!("o{k}")
}

fn kind_of(spec: &Spec) -> &'static str {
    match spec {
        Spec::Noul { .. } => "noul",
        Spec::Choice { .. } => "choice",
        Spec::Score { .. } => "score",
        Spec::Box { .. } => "box",
    }
}

fn questions_for(specs: &[Spec]) -> Questions {
    let mut questions = Questions::new();
    for (i, spec) in specs.iter().enumerate() {
        match spec {
            Spec::Noul { .. } => {
                questions.noul(id(i), "?", None).unwrap();
            }
            Spec::Choice { options, .. } => {
                questions
                    .dynamic_choice(id(i), "?", (0..*options).map(|k| (option(k), None)))
                    .unwrap();
            }
            Spec::Score { levels, .. } => {
                questions.score(id(i), "?", levels.clone()).unwrap();
            }
            Spec::Box { .. } => {
                questions.r#box(id(i), "where").unwrap();
            }
        }
    }
    questions
}

fn prob(p: f64) -> Probability {
    Probability::new(p).unwrap()
}

fn conf(c: f64) -> Confidence {
    Confidence::new(c).unwrap()
}

/// The answer that fits `spec`'s question. A Score has at most ten levels,
/// so its top index is exact as a float.
#[allow(clippy::cast_precision_loss)]
fn fitting_answer(spec: &Spec) -> Answer {
    match spec {
        Spec::Noul { p } => Answer::Noul {
            noul: prob(*p),
            extra: BTreeMap::new(),
        },
        Spec::Choice {
            options,
            pick,
            p,
            confidence,
        } => Answer::Choice {
            choice: option(*pick),
            probabilities: (0..*options).map(|k| (option(k), prob(*p))).collect(),
            confidence: conf(*confidence),
            extra: BTreeMap::new(),
        },
        Spec::Score {
            levels,
            echo_as_text,
            at,
            p,
            confidence,
        } => {
            let top = (levels.len() - 1) as f64;
            Answer::Score {
                score: at * top,
                // A structured level may come back as itself or as its text.
                legend: levels
                    .iter()
                    .zip(echo_as_text)
                    .enumerate()
                    .map(|(i, (level, as_text))| {
                        let echoed = if *as_text && !level.is_string() {
                            Value::String(level.to_string())
                        } else {
                            level.clone()
                        };
                        (i.to_string(), echoed)
                    })
                    .collect(),
                probabilities: (0..levels.len())
                    .map(|i| (i.to_string(), prob(*p)))
                    .collect(),
                confidence: conf(*confidence),
                extra: BTreeMap::new(),
            }
        }
        Spec::Box { top, left } => Answer::Box {
            edges: BoxEdges::new(*top, top + 0.5, *left, left + 0.5),
            confidence: None,
            extra: BTreeMap::new(),
        },
    }
}

fn response_for(specs: &[Spec]) -> Response {
    Response {
        model: "jev-test".to_owned(),
        answers: specs
            .iter()
            .enumerate()
            .map(|(i, spec)| (id(i), fitting_answer(spec)))
            .collect(),
        usage: Usage::default(),
        request_id: None,
        extra: BTreeMap::new(),
    }
}

proptest! {
#![proptest_config(config())]
    #[test]
    fn a_fitting_response_verifies_and_an_extra_answer_is_tolerated(
        specs in prop::collection::vec(any_spec(), 1..6),
    ) {
        let questions = questions_for(&specs);
        let mut response = response_for(&specs);
        prop_assert!(response.verify(&questions).is_ok());
        response.answers.insert("not-asked".to_owned(), Answer::Noul { noul: prob(0.5), extra: BTreeMap::new(), });
        prop_assert!(response.verify(&questions).is_ok());
    }

    #[test]
    fn a_dropped_answer_is_the_missing_answer_of_that_question(
        specs in prop::collection::vec(any_spec(), 1..6),
        which in any::<usize>(),
    ) {
        let questions = questions_for(&specs);
        let mut response = response_for(&specs);
        let dropped = id(which % specs.len());
        response.answers.remove(&dropped);
        match response.verify(&questions) {
            Err(Error::MissingAnswer { id, .. }) => prop_assert_eq!(id, dropped),
            other => prop_assert!(false, "{other:?}"),
        }
    }

    #[test]
    fn an_option_that_was_not_offered_is_an_unknown_option(
        (specs, at) in specs_with(choice_spec()),
        in_the_distribution in any::<bool>(),
    ) {
        let questions = questions_for(&specs);
        let mut response = response_for(&specs);
        let Some(Answer::Choice { choice, probabilities, .. }) = response.answers.get_mut(&id(at))
        else {
            panic!("question {at} is a Choice");
        };
        if in_the_distribution {
            let (key, p) = probabilities.pop_first().unwrap();
            let _ = key;
            probabilities.insert(BOGUS.to_owned(), p);
        } else {
            BOGUS.clone_into(choice);
        }
        match response.verify(&questions) {
            Err(Error::UnknownOption { id: found, option, .. }) => {
                prop_assert_eq!(found, id(at));
                prop_assert_eq!(option, BOGUS);
            }
            other => prop_assert!(false, "{other:?}"),
        }
    }

    #[test]
    fn a_legend_that_is_not_the_levels_sent_is_an_invalid_answer(
        (specs, at) in specs_with(score_spec()),
        how in 0u8..3,
        entry in any::<usize>(),
    ) {
        let questions = questions_for(&specs);
        let mut response = response_for(&specs);
        let Some(Answer::Score { legend, .. }) = response.answers.get_mut(&id(at)) else {
            panic!("question {at} is a Score");
        };
        let levels = legend.len();
        match how {
            // One level's text is not the one sent.
            0 => {
                legend.insert((entry % levels).to_string(), Value::String(BOGUS.to_owned()));
            }
            // A level is missing.
            1 => {
                legend.remove(&(entry % levels).to_string());
            }
            // A level the question never sent.
            _ => {
                legend.insert(levels.to_string(), Value::String("one more".to_owned()));
            }
        }
        match response.verify(&questions) {
            Err(Error::InvalidAnswer { id: found, .. }) => prop_assert_eq!(found, id(at)),
            other => prop_assert!(false, "{other:?}"),
        }
    }

    #[test]
    fn an_answer_of_another_kind_is_a_type_mismatch(
        specs in prop::collection::vec(any_spec(), 1..6),
        which in any::<usize>(),
    ) {
        let at = which % specs.len();
        let questions = questions_for(&specs);
        let mut response = response_for(&specs);
        // Any kind but the question's.
        let wrong = if matches!(specs[at], Spec::Noul { .. }) {
            Answer::Choice {
                choice: "x".to_owned(),
                probabilities: BTreeMap::from([("x".to_owned(), prob(1.0))]),
                confidence: conf(1.0), extra: BTreeMap::new(), }
        } else {
            Answer::Noul { noul: prob(0.5), extra: BTreeMap::new(), }
        };
        let actual_kind = wrong.kind().to_owned();
        response.answers.insert(id(at), wrong);
        match response.verify(&questions) {
            Err(Error::AnswerTypeMismatch { id: found, expected, actual, .. }) => {
                prop_assert_eq!(found, id(at));
                prop_assert_eq!(expected, kind_of(&specs[at]));
                prop_assert_eq!(actual, actual_kind);
            }
            other => prop_assert!(false, "{other:?}"),
        }
    }
}

options! {
    /// The options a generated three-option Choice offers (`option(0..3)`),
    /// so a typed handle reads exactly those questions.
    enum Three {
        O0 = "o0" => None,
        O1 = "o1" => "the second",
        O2 = "o2" => None,
    }
}

/// The `extra` map every known answer carries.
fn extra_of(answer: &mut Answer) -> &mut BTreeMap<String, Value> {
    match answer {
        Answer::Noul { extra, .. }
        | Answer::Choice { extra, .. }
        | Answer::Score { extra, .. }
        | Answer::Box { extra, .. } => extra,
        other => panic!("not a known answer: {other:?}"),
    }
}

/// What `src/answer.rs` allows a score past either end of its scale
/// (`SCORE_EPSILON`, private there).
const SCORE_MARGIN: f64 = 1e-9;

proptest! {
#![proptest_config(config())]
    /// The guarantee `Response::get` documents: after `verify`, a handle
    /// from the same questions cannot fail, whatever else the answer holds.
    #[test]
    fn after_verify_every_handle_from_the_questions_reads(
        specs in prop::collection::vec(any_spec(), 1..6),
        thin in prop::collection::vec(any::<bool>(), 6),
    ) {
        let questions = questions_for(&specs);
        let mut response = response_for(&specs);
        for (i, answer) in response.answers.values_mut().enumerate() {
            // An undocumented field, as Laya's `action`, and a Choice
            // distribution missing every option but the chosen one.
            extra_of(answer).insert("action".to_owned(), json!({ "n": i }));
            if let (true, Answer::Choice { choice, probabilities, .. }) = (thin[i], &mut *answer) {
                probabilities.retain(|key, _| key == choice);
            }
        }
        prop_assert!(response.verify(&questions).is_ok());
        #[allow(clippy::cast_precision_loss)]
        for (i, spec) in specs.iter().enumerate() {
            let id = id(i);
            match spec {
                Spec::Noul { p } => {
                    let read = response.get(&questions.handle::<Noul>(&id).unwrap()).unwrap();
                    prop_assert_eq!(read.yes.value().to_bits(), p.to_bits());
                }
                Spec::Choice { options, pick, .. } => {
                    let read = response.get(&questions.handle::<Choice<String>>(&id).unwrap()).unwrap();
                    prop_assert_eq!(read.chosen, option(*pick));
                    // A typed handle exists exactly when the enum is the options offered.
                    let typed = questions.handle::<Choice<Three>>(&id);
                    prop_assert_eq!(typed.is_some(), *options == 3);
                    if let Some(handle) = typed {
                        let read = response.get(&handle).unwrap();
                        prop_assert_eq!(read.chosen.key(), option(*pick));
                        prop_assert_eq!(read.probabilities.len(), 3);
                    }
                }
                Spec::Score { levels, at, .. } => {
                    let read = response.get(&questions.handle::<Score>(&id).unwrap()).unwrap();
                    prop_assert_eq!(read.levels.len(), levels.len());
                    prop_assert_eq!(read.probabilities.len(), levels.len());
                    // In level order, `"10"` after `"9"`: each label is its level's.
                    for (label, level) in read.levels.iter().zip(levels) {
                        match level {
                            Value::String(text) => prop_assert_eq!(label, text),
                            structured => prop_assert_eq!(label, &structured.to_string()),
                        }
                    }
                    prop_assert_eq!(read.value.to_bits(), (at * (levels.len() - 1) as f64).to_bits());
                }
                Spec::Box { .. } => {
                    response.get(&questions.handle::<BoundingBox>(&id).unwrap()).unwrap();
                }
            }
        }
    }

    /// With several answers wrong, the one reported is the first in wire
    /// order (the order the questions were added), not the lowest id: the
    /// ids here are added shuffled.
    #[test]
    fn verify_reports_the_first_failure_in_wire_order(
        specs in prop::collection::vec(any_spec(), 2..6),
        names in Just((0..6).collect::<Vec<usize>>()).prop_shuffle(),
        broken in prop::collection::vec(prop_oneof![Just(None), Just(Some(true)), Just(Some(false))], 6),
        forced in any::<usize>(),
    ) {
        let base = questions_for(&specs);
        let fitting = response_for(&specs);
        let mut broken = broken;
        // At least one answer is wrong.
        broken[forced % specs.len()].get_or_insert(true);
        let mut questions = Questions::new();
        let mut response = fitting.clone();
        response.answers.clear();
        let mut expected = None;
        for (i, (old, question)) in base.iter().enumerate() {
            let name = format!("q{}", names[i]);
            questions.add(name.clone(), question.clone()).unwrap();
            match broken[i] {
                None => {
                    response.answers.insert(name, fitting.answers[old].clone());
                }
                Some(missing) => {
                    expected.get_or_insert((name.clone(), missing));
                    if !missing {
                        let wrong = if matches!(question, judgment::Question::Noul { .. }) {
                            fitting_answer(&Spec::Box { top: 0.1, left: 0.1 })
                        } else {
                            fitting_answer(&Spec::Noul { p: 0.5 })
                        };
                        response.answers.insert(name, wrong);
                    }
                }
            }
        }
        let (first, missing) = expected.unwrap();
        match response.verify(&questions) {
            Err(Error::MissingAnswer { id, .. }) if missing => prop_assert_eq!(id, first),
            Err(Error::AnswerTypeMismatch { id, .. }) if !missing => prop_assert_eq!(id, first),
            other => prop_assert!(false, "expected {first} ({missing}): {other:?}"),
        }
    }

    /// The offered set is matched by exact string: a key one space, one
    /// case or one Unicode normalisation away from an offered option is not
    /// it. The oracle is membership, so a mutation that lands on another
    /// offered option (or changes nothing) must verify.
    #[test]
    fn an_option_near_an_offered_one_is_offered_only_if_it_is_one(
        offered in prop::collection::btree_set("(?:[a-z]|é|e\u{301}|ß){1,5}", 2..5),
        which in any::<usize>(),
        how in 0u8..6,
        in_the_distribution in any::<bool>(),
    ) {
        let offered: Vec<String> = offered.into_iter().collect();
        let near = &offered[which % offered.len()];
        let mutated = match how {
            0 => format!(" {near}"),
            1 => format!("{near}\u{a0}"),
            2 => near.to_uppercase(),
            3 => near.replace('é', "e\u{301}"),
            4 => near.replace("e\u{301}", "é"),
            _ => near.replace('ß', "ss"),
        };
        let mut questions = Questions::new();
        questions
            .dynamic_choice("c", "?", offered.iter().map(|k| (k.clone(), None)))
            .unwrap();
        let (choice, key) = if in_the_distribution {
            (offered[0].clone(), mutated.clone())
        } else {
            (mutated.clone(), offered[0].clone())
        };
        let response = Response {
            model: "jev-test".to_owned(),
            answers: BTreeMap::from([(
                "c".to_owned(),
                Answer::Choice {
                    choice,
                    probabilities: BTreeMap::from([(key, prob(1.0))]),
                    confidence: conf(1.0),
                    extra: BTreeMap::new(),
                },
            )]),
            usage: Usage::default(),
            request_id: None,
            extra: BTreeMap::new(),
        };
        match response.verify(&questions) {
            Ok(()) => prop_assert!(offered.contains(&mutated), "{mutated:?} is not offered"),
            Err(Error::UnknownOption { option, .. }) => {
                prop_assert!(!offered.contains(&mutated), "{mutated:?} is offered");
                prop_assert_eq!(option, mutated);
            }
            Err(other) => prop_assert!(false, "{other:?}"),
        }
    }

    /// A Score is on its scale up to the margin past either end and off it
    /// beyond, at every number of levels; a probability key is a level only
    /// as the exact decimal index.
    #[test]
    fn a_score_is_held_to_its_scale_and_its_exact_level_keys(
        (specs, at) in specs_with(score_spec()),
        top_end in any::<bool>(),
        offset in prop::sample::select(vec![0.0, 0.5e-9, -0.5e-9, 2e-9, -2e-9, 1e-3, -1e-3]),
        key in 0usize..7,
        level in any::<usize>(),
    ) {
        let questions = questions_for(&specs);
        let mut response = response_for(&specs);
        let Some(Answer::Score { score, probabilities, .. }) = response.answers.get_mut(&id(at)) else {
            panic!("question {at} is a Score");
        };
        let n = probabilities.len();
        #[allow(clippy::cast_precision_loss)]
        let top = (n - 1) as f64;
        let value = if top_end { top + offset } else { offset };
        *score = value;
        let outward = if top_end { offset } else { -offset };
        let on_scale = outward <= 0.0 || outward < SCORE_MARGIN;
        prop_assert_eq!(response.verify(&questions).is_ok(), on_scale, "{} on 0..={}", value, top);

        // Back on the scale, then a key that parses as a level and is not one.
        let Some(Answer::Score { score, probabilities, .. }) = response.answers.get_mut(&id(at)) else {
            unreachable!();
        };
        *score = 0.0;
        let i = level % n;
        let off = match key {
            0 => format!("0{i}"),
            1 => format!("+{i}"),
            2 => format!(" {i}"),
            3 => format!("{i} "),
            4 => format!("{i}.0"),
            5 => format!("{i}\u{0}"),
            _ => n.to_string(),
        };
        probabilities.insert(off.clone(), prob(0.0));
        match response.verify(&questions) {
            Err(Error::InvalidAnswer { id: found, reason, .. }) => {
                prop_assert_eq!(found, id(at));
                prop_assert!(reason.starts_with("probability key"), "{}", reason);
            }
            other => prop_assert!(false, "{off:?}: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Decoding a response
// ---------------------------------------------------------------------------

/// Strings a server could send where a kind or a field name goes, the
/// documented ones and their near misses among them.
fn wire_word() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::sample::select(vec![
            "noul",
            "choice",
            "score",
            "box",
            "Choice",
            "NOUL",
            " noul",
            "rank",
            "type",
            "probabilities",
            "confidence",
            "legend",
            "top",
            "bottom",
            "left",
            "right",
            "0",
            "1",
            "01",
            "model",
            "answers",
            "usage",
            "input_tokens",
            "output_tokens",
            "cost",
            "request_id",
            "id",
            "action",
        ])
        .prop_map(str::to_owned),
        "\\PC{0,4}",
    ]
}

/// A JSON value to depth three, numbers biased to the unit interval and its
/// edges, keys from [`wire_word`].
fn wire_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        1 => Just(Value::Null),
        1 => any::<bool>().prop_map(Value::from),
        1 => any::<i64>().prop_map(Value::from),
        1 => any::<u64>().prop_map(Value::from),
        3 => any_float().prop_filter_map("not a JSON number", |f| {
            serde_json::Number::from_f64(f).map(Value::Number)
        }),
        1 => wire_word().prop_map(Value::from),
    ];
    leaf.prop_recursive(3, 24, 5, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::from),
            prop::collection::btree_map(wire_word(), inner, 0..5)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

/// An answer-shaped object: one that fits a question, then as often as not
/// a field set to anything (the `type` among them: a kind, a near miss or
/// not a string), a field dropped, a field added.
fn wire_answer() -> impl Strategy<Value = Value> {
    (
        any_spec(),
        prop::option::of((
            prop_oneof![Just("type".to_owned()), wire_word(),],
            prop_oneof![
                2 => wire_value(),
                1 => prop::sample::select(vec![Value::Null, json!(5), json!("Choice")]),
            ],
        )),
        prop::option::of(wire_word()),
        prop::option::of((wire_word(), wire_value())),
    )
        .prop_map(|(spec, set, dropped, added)| {
            let mut answer = serde_json::to_value(fitting_answer(&spec)).unwrap();
            let fields = answer.as_object_mut().unwrap();
            if let Some((key, value)) = set {
                fields.insert(key, value);
            }
            if let Some(key) = dropped {
                fields.remove(&key);
            }
            if let Some((key, value)) = added {
                fields.entry(key).or_insert(value);
            }
            answer
        })
}

/// A response-shaped object over [`wire_answer`]s, or any value at all.
fn wire_response() -> impl Strategy<Value = Value> {
    prop_oneof![
        4 => (
            prop::collection::btree_map(wire_word(), wire_value(), 0..4),
            prop::collection::btree_map("q[0-3]", wire_answer(), 0..4),
            any::<bool>(),
        )
            .prop_map(|(mut top, answers, named)| {
                if named {
                    top.insert("model".to_owned(), json!("jev-test"));
                }
                top.insert("answers".to_owned(), Value::Object(answers.into_iter().collect()));
                Value::Object(top.into_iter().collect())
            }),
        1 => wire_value(),
    ]
}

/// Every probability and confidence `answer` holds, as numbers.
fn unit_values(answer: &Answer) -> Vec<f64> {
    match answer {
        Answer::Noul { noul, .. } => vec![noul.value()],
        Answer::Choice {
            probabilities,
            confidence,
            ..
        }
        | Answer::Score {
            probabilities,
            confidence,
            ..
        } => probabilities
            .values()
            .map(|p| p.value())
            .chain([confidence.value()])
            .collect(),
        Answer::Box { confidence, .. } => confidence.iter().map(|c| c.value()).collect(),
        _ => Vec::new(),
    }
}

proptest! {
#![proptest_config(config())]
    /// Decoding is total (a value or an error, never a panic), a decoded
    /// probability or confidence is in `[0, 1]`, the kind is the `type` sent
    /// exactly (a near miss is an unknown kind kept as it came, a `type`
    /// that is not a string an error), and what decodes writes back to
    /// itself.
    #[test]
    fn decoding_is_total_keeps_the_unit_interval_and_the_kind_sent(raw in wire_response()) {
        // Each answer alone, so one that fails does not hide the others.
        let sent_answers = raw.get("answers").and_then(Value::as_object).cloned().unwrap_or_default();
        let mut decoded = BTreeMap::new();
        for (id, sent) in &sent_answers {
            let Ok(answer) = serde_json::from_str::<Answer>(&sent.to_string()) else {
                continue;
            };
            for x in unit_values(&answer) {
                prop_assert!(in_unit_interval(x), "{id}: {x}");
            }
            match sent["type"].as_str() {
                Some(kind @ ("noul" | "choice" | "score" | "box")) => {
                    prop_assert_eq!(answer.kind(), kind);
                    // The tag is the variant's, never a stray field written back twice.
                    prop_assert!(!extra_of(&mut answer.clone()).contains_key("type"));
                }
                Some(_) => prop_assert_eq!(&answer, &Answer::Unknown(sent.clone())),
                None => prop_assert!(false, "{id} decoded without a string type: {sent}"),
            }
            let again = serde_json::from_str::<Answer>(&serde_json::to_string(&answer).unwrap());
            prop_assert_eq!(again.unwrap(), answer.clone());
            decoded.insert(id.clone(), answer);
        }
        if let Ok(response) = serde_json::from_str::<Response>(&raw.to_string()) {
            // A response holds its answers as each decodes alone, and writes back to itself.
            prop_assert_eq!(&response.answers, &decoded);
            let again = serde_json::from_str::<Response>(&serde_json::to_string(&response).unwrap());
            prop_assert_eq!(again.unwrap(), response);
        }
    }
}

// ---------------------------------------------------------------------------
// The limits on Questions
// ---------------------------------------------------------------------------

/// Counts around the edges as often as the middle.
fn around(low: usize, high: usize, most: usize) -> impl Strategy<Value = usize> {
    prop_oneof![
        3 => 0..=most,
        1 => Just(low - 1),
        1 => Just(low),
        1 => Just(high),
        1 => Just(high + 1),
    ]
}

/// One valid question of the kind `kind % 4`, under `id`.
fn add(questions: &mut Questions, id: &str, kind: usize) -> judgment::Result<()> {
    match kind % 4 {
        0 => questions.noul(id, "?", None).map(drop),
        1 => questions
            .dynamic_choice(id, "?", [("a".to_owned(), None), ("b".to_owned(), None)])
            .map(drop),
        2 => questions.score(id, "?", ["low", "high"]).map(drop),
        _ => questions.r#box(id, "where").map(drop),
    }
}

proptest! {
#![proptest_config(config())]
    #[test]
    fn a_choice_takes_two_to_255_options(n in around(2, 255, 300)) {
        let mut questions = Questions::new();
        let result = questions.dynamic_choice("c", "?", (0..n).map(|i| (option(i), None)));
        prop_assert_eq!(result.is_ok(), (2..=255).contains(&n), "{} options", n);
        if let Err(error) = result {
            prop_assert!(matches!(error, Error::InvalidQuestion { .. }), "{error:?}");
            prop_assert!(questions.is_empty(), "a refused question was added");
        }
    }

    #[test]
    fn a_score_takes_two_to_ten_levels(n in around(2, 10, 20)) {
        let mut questions = Questions::new();
        let result = questions.score("s", "?", (0..n).map(|i| format!("level {i}")));
        prop_assert_eq!(result.is_ok(), (2..=10).contains(&n), "{} levels", n);
        if let Err(error) = result {
            prop_assert!(matches!(error, Error::InvalidQuestion { .. }), "{error:?}");
            prop_assert!(questions.is_empty(), "a refused question was added");
        }
    }

    #[test]
    fn a_repeated_id_is_refused_and_the_first_question_stays(
        name in "[a-z]{1,6}",
        first in 0usize..4,
        second in 0usize..4,
    ) {
        let mut questions = Questions::new();
        add(&mut questions, &name, first).unwrap();
        let kept = questions.get(&name).unwrap().kind();
        match add(&mut questions, &name, second) {
            Err(Error::DuplicateQuestionId(repeated)) => prop_assert_eq!(repeated, name.clone()),
            other => prop_assert!(false, "{other:?}"),
        }
        prop_assert_eq!(questions.len(), 1);
        prop_assert_eq!(questions.get(&name).unwrap().kind(), kept);
    }

    #[test]
    fn an_empty_id_is_refused_for_every_kind(kind in 0usize..4) {
        let mut questions = Questions::new();
        match add(&mut questions, "", kind) {
            Err(Error::InvalidQuestion { id, .. }) => prop_assert!(id.is_empty()),
            other => prop_assert!(false, "{other:?}"),
        }
        prop_assert!(questions.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Box edges: the overlap and the pixels
// ---------------------------------------------------------------------------

/// A region: each pair ordered and inside the image, never thinner than a
/// millionth, so its area does not underflow to nothing.
fn region() -> impl Strategy<Value = BoxEdges> {
    let span = || (0.0f64..1.0, 1e-6f64..1.0).prop_map(|(lo, w)| (lo, (lo + w).min(1.0)));
    (span(), span()).prop_filter_map("thinner than a millionth", |((t, b), (l, r))| {
        (b - t >= 1e-6 && r - l >= 1e-6).then(|| BoxEdges::new(t, b, l, r))
    })
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9
}

proptest! {
#![proptest_config(config())]
    #[test]
    fn overlap_is_symmetric_bounded_and_one_for_a_box_with_itself(
        a in region(), b in region(),
    ) {
        let (ab, ba) = (a.iou(&b), b.iou(&a));
        prop_assert!(close(ab, ba), "{ab} vs {ba}");
        prop_assert!(in_unit_interval(ab), "{ab}");
        prop_assert!(close(a.iou(&a), 1.0), "{}", a.iou(&a));
    }

    #[test]
    fn two_boxes_that_do_not_touch_have_no_overlap(
        a in region(), b in region(),
    ) {
        // `b`'s span set down against `a`'s right edge, exactly, so the two
        // share an edge and nothing else; shifting would round into an overlap.
        let moved = BoxEdges::new(b.top, b.bottom, a.right, (a.right + (b.right - b.left)).min(1.0));
        prop_assume!(moved.right > moved.left);
        prop_assert_eq!(a.iou(&moved).to_bits(), 0.0f64.to_bits());
    }

    #[test]
    fn pixels_round_to_the_nearest_keep_the_order_and_stay_inside(
        edges in region(), width in 1u32..4096, height in 1u32..4096,
    ) {
        let px = edges.to_pixels(width, height);
        prop_assert!(px.left <= px.right && px.top <= px.bottom, "{px:?}");
        prop_assert!(px.right <= width && px.bottom <= height, "{px:?}");
        for (pixel, edge, size) in [
            (px.left, edges.left, width),
            (px.right, edges.right, width),
            (px.top, edges.top, height),
            (px.bottom, edges.bottom, height),
        ] {
            let back = f64::from(pixel) / f64::from(size);
            prop_assert!(
                (back - edge).abs() <= 0.5 / f64::from(size) + f64::EPSILON,
                "{edge} -> {pixel} of {size} -> {back}"
            );
        }
    }

    #[test]
    fn pixels_never_leave_the_image_whatever_the_edges(
        top in any_float(), bottom in any_float(), left in any_float(), right in any_float(),
        width in 1u32..4096, height in 1u32..4096,
    ) {
        let px = BoxEdges::new(top, bottom, left, right).to_pixels(width, height);
        prop_assert!(px.left <= width && px.right <= width, "{px:?}");
        prop_assert!(px.top <= height && px.bottom <= height, "{px:?}");
    }
}
