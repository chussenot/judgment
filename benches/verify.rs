//! What a call costs outside the network: holding a response against its
//! questions, fingerprinting a request, finding a recording and answering
//! from a script. `mise run bench` runs them; the numbers and the machine
//! they came from are in docs/reference/performance.md.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use divan::black_box;
use judgment::backend::{Fake, Replay};
use judgment::eval::Recording;
use judgment::eval::canonical::{self, request_fingerprint};
use judgment::{Questions, Response, SystemOne};
use serde_json::{Value, json};

fn main() {
    divan::main();
}

fn id(i: usize) -> String {
    format!("q{i}")
}

/// `n` questions, a Noul, a Choice and a Score in turn.
fn questions(n: usize) -> Questions {
    let mut questions = Questions::new();
    for i in 0..n {
        match i % 3 {
            0 => questions.noul(id(i), "?", None).map(drop),
            1 => questions
                .dynamic_choice(id(i), "?", [("a".to_owned(), None), ("b".to_owned(), None)])
                .map(drop),
            _ => questions.score(id(i), "?", ["low", "high"]).map(drop),
        }
        .unwrap();
    }
    questions
}

/// The wire answers to [`questions`].
fn response(n: usize) -> Response {
    let answers: serde_json::Map<String, Value> = (0..n)
        .map(|i| {
            let answer = match i % 3 {
                0 => json!({"type": "noul", "noul": 0.5}),
                1 => json!({"type": "choice", "choice": "a",
                            "probabilities": {"a": 0.5, "b": 0.5}, "confidence": 0.5}),
                _ => json!({"type": "score", "score": 0.5, "legend": {"0": "low", "1": "high"},
                            "probabilities": {"0": 0.5, "1": 0.5}, "confidence": 0.5}),
            };
            (id(i), answer)
        })
        .collect();
    serde_json::from_value(json!({
        "model": "bench", "answers": answers,
        "usage": {"input_tokens": 100, "output_tokens": 10}
    }))
    .unwrap()
}

/// `Response::verify` over 1, 16 and 64 questions.
#[divan::bench(args = [1, 16, 64])]
fn verify(bencher: divan::Bencher, n: usize) {
    let (questions, response) = (questions(n), response(n));
    bencher.bench_local(|| black_box(&response).verify(black_box(&questions)).unwrap());
}

/// A JSON object of about `bytes` bytes, many short fields.
fn json_of(bytes: usize) -> Value {
    let fields = bytes / 32;
    Value::Object(
        (0..fields)
            .map(|i| (format!("field_{i:05}"), Value::String("x".repeat(12))))
            .collect(),
    )
}

/// The canonical fingerprint (RFC 8785, SHA-256) of 1 KiB and 64 KiB of state.
#[divan::bench(args = [1 << 10, 64 << 10])]
fn fingerprint(bencher: divan::Bencher, bytes: usize) {
    let state = json_of(bytes);
    bencher.bench_local(|| canonical::fingerprint(black_box(&state)));
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
}

/// Finding and verifying the recording for a 16-question request.
#[divan::bench]
fn replay_lookup(bencher: divan::Bencher) {
    let (state, questions) = (json!({"message": "hello"}), questions(16));
    let mut replay = Replay::default();
    let mut recording = Recording::new("case", response(16), 1);
    recording.fingerprint = Some(request_fingerprint(&state, &questions));
    replay.add(recording);
    let runtime = runtime();
    bencher.bench_local(|| {
        runtime
            .block_on(replay.answer(black_box(&state), "bench", black_box(&questions)))
            .unwrap()
    });
}

/// A `Fake` answering 16 questions, which is a verified response built from a script.
#[divan::bench]
fn fake_round_trip(bencher: divan::Bencher) {
    let (state, questions) = (json!({"message": "hello"}), questions(16));
    let mut fake = Fake::new();
    for i in 0..16 {
        fake = match i % 3 {
            0 => fake.noul(id(i), 0.5).unwrap(),
            1 => fake.choice(id(i), [("a", 0.5), ("b", 0.5)], 0.5).unwrap(),
            _ => fake.score(id(i), [0.5, 0.5], 0.5).unwrap(),
        };
    }
    let runtime = runtime();
    bencher.bench_local(|| {
        runtime
            .block_on(fake.answer(black_box(&state), "bench", black_box(&questions)))
            .unwrap()
    });
}
