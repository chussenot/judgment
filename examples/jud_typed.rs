//! A `.jud` rubric wired into a Rust program as a typed module: `mod triage;`
//! and one call. `examples/jud/triage.rs` is what the jud plugin's
//! `/jud:rust` writes for `examples/jud/triage.jud`: the rubric is read from
//! the file at compile time, so its requests are the ones `jud lower` sends,
//! and its answers come back as enums the compiler checks. Here the answers
//! are the recordings under `examples/recordings/jud_calibration/`, replayed
//! without a key; swap the [`Replay`] for a `Client` to ask a model.
//!
//! ```sh
//! cargo run -p judgment --features jud --example jud_typed
//! ```

#![allow(clippy::print_stdout)]

use std::path::Path;

use judgment::Replay;
use judgment::jud::Cases;

#[path = "jud/triage.rs"]
mod triage;

// A rubric whose options come with each request and whose questions depend
// on the state; exercised by the tests below.
#[path = "jud/routing.rs"]
mod routing;

use triage::{Desk, Gated, Tone};

/// What the program does with a message: the decision is the rubric's, the
/// routing is the program's own.
fn route(decision: &triage::Decision) -> String {
    if !decision.actionable.yes {
        return "archive".to_owned();
    }
    let desk = match decision.desk.or_fallback() {
        Some(Desk::Billing) => "billing",
        Some(Desk::Technical) => "technical",
        Some(Desk::Account) => "account",
        Some(Desk::NoneOfThese) | None => "front desk",
    };
    let urgent = matches!(decision.tone, Gated::Act { value, .. } if value >= Tone::Angry);
    format!("{desk}{}", if urgent { ", urgent" } else { "" })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let backend = Replay::open(&root.join("examples/recordings/jud_calibration"))?;
    let cases = Cases::parse(&std::fs::read_to_string(
        root.join("examples/jud/triage-cases.jud"),
    )?)?;
    for (i, case) in cases.cases.iter().enumerate() {
        let decision = triage::decide(&backend, "jev-latest", &case.state).await?;
        println!("{:<14} {}", case.name(i), route(&decision));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use judgment::jud::Expect;

    use super::*;

    /// Every labelled case decides from the recordings, and the typed
    /// decision agrees with the rubric's own verdicts.
    #[tokio::test]
    async fn every_case_decides_from_the_recordings() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let backend = Replay::open(&root.join("examples/recordings/jud_calibration")).unwrap();
        let text = std::fs::read_to_string(root.join("examples/jud/triage-cases.jud")).unwrap();
        let cases = Cases::parse(&text).unwrap();
        for (i, case) in cases.cases.iter().enumerate() {
            let decision = triage::decide(&backend, "jev-latest", &case.state)
                .await
                .unwrap_or_else(|e| panic!("{}: {e}", case.name(i)));
            if matches!(case.expect.get("actionable"), Some(Expect::Bool(false))) {
                assert_eq!(route(&decision), "archive", "{}", case.name(i));
            }
        }
    }

    #[test]
    fn the_module_matches_the_rubric() {
        assert_eq!(
            triage::rubric().unwrap().fingerprint(),
            triage::QUESTIONS_FINGERPRINT
        );
    }

    /// Every routing case lowers to the same request through the module as
    /// through the rubric, so the module and `jud lower` send the same bytes.
    #[test]
    fn the_routing_module_sends_what_the_rubric_sends() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(root.join("examples/jud/routing-cases.jud")).unwrap();
        let rubric = routing::rubric().unwrap();
        for case in &Cases::parse(&text).unwrap().cases {
            let pairs = |id: &str| -> Vec<(String, String)> {
                case.options.get(id).map_or_else(Vec::new, |options| {
                    options
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                        .collect()
                })
            };
            let offered = routing::Offered {
                desk: pairs("desk"),
                duplicate_of: pairs("duplicate_of"),
            };
            assert_eq!(
                routing::questions(&case.state, &offered).unwrap(),
                rubric.lower(&case.state, &case.options).unwrap()
            );
        }
    }

    /// Bands, a strict threshold, `level_at_least` and the questions a state
    /// leaves out, read through the typed module.
    #[tokio::test]
    async fn the_routing_module_types_every_gate() {
        let state = serde_json::json!({
            "message": {"text": "Charged twice for #1042, refund it now."},
            "customer": {"recent_orders": ["#1042"], "open_tickets": [{"id": "T-1"}]}
        });
        let offered = routing::Offered {
            desk: vec![
                ("billing".into(), "Invoices, payments, refunds".into()),
                ("technical".into(), "Errors and how-to questions".into()),
            ],
            duplicate_of: vec![("T-1".into(), "Charged twice".into())],
        };
        let backend = judgment::Fake::new()
            .choice(
                "desk",
                [
                    ("billing", 0.55),
                    ("technical", 0.25),
                    ("none_of_these", 0.2),
                ],
                0.55,
            )
            .unwrap()
            .score("tone", [0.0, 0.1, 0.8, 0.1], 0.8)
            .unwrap()
            .choice("duplicate_of", [("T-1", 0.5), ("none", 0.5)], 0.5)
            .unwrap()
            .noul("refund_request", 0.65)
            .unwrap();
        let d = routing::decide(&backend, "jev-latest", &state, &offered)
            .await
            .unwrap();
        assert!(
            matches!(&d.desk, routing::Gated::Act { value: routing::Desk::Supplied(k), band: Some(b), .. }
            if k == "billing" && b == "confirm")
        );
        assert!(matches!(
            d.tone,
            routing::Gated::Act {
                value: routing::Tone::Angry,
                reached: Some(true),
                ..
            }
        ));
        assert_eq!(
            d.duplicate_of.and_then(|g| g.or_fallback()),
            Some(routing::DuplicateOf::None),
            "0.5 is under the 0.75 bar: the fallback"
        );
        assert_eq!(
            d.refund_request.map(|r| r.yes),
            Some(false),
            "strict: 0.65 is not above 0.65"
        );

        let bare = serde_json::json!({"message": {"text": "Hello"}});
        let backend = judgment::Fake::new()
            .choice("desk", [("billing", 0.9), ("none_of_these", 0.1)], 0.9)
            .unwrap()
            .score("tone", [0.9, 0.1, 0.0, 0.0], 0.9)
            .unwrap();
        let offered = routing::Offered {
            desk: vec![("billing".into(), "Invoices".into())],
            ..routing::Offered::default()
        };
        let d = routing::decide(&backend, "jev-latest", &bare, &offered)
            .await
            .unwrap();
        assert_eq!((d.duplicate_of, d.refund_request), (None, None));
    }
}
