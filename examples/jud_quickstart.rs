//! The README's second example: the two questions of `quickstart.rs` as a
//! `.jud` rubric with the thresholds that read their answers, answered by
//! the `Fake` backend and read through the rubric's gates. It needs the `jud`
//! feature and no key or network. `tests/readme.rs` holds the README's copy
//! to this file.
//!
//! ```text
//! cargo run --example jud_quickstart --features jud
//! ```

// README:BEGIN
use judgment::jud::{Rubric, Supplied, Verdict};
use judgment::{Fake, SystemOne};

// The same questions as above, with the thresholds beside them, in a file
// another tool can read (docs/jud.md). The policy is never sent to the model.
const RUBRIC: &str = r"
jud: 1.2
kind: rubric
id: inbox-triage
questions:
  department:
    type: choice
    instructions: Which team should handle `message`?
    criteria:
      billing: Payments, invoicing, refunds
      technical: Bugs, outages, integrations
  is_urgent:
    type: noul
    instructions: Does `message` convey urgency?
policy:
  department:
    confidence: 0.7
    fallback: technical
  is_urgent:
    threshold: 0.6
";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parsing checks the rubric as the builder checks questions; lowering
    // builds the request for this state.
    let rubric = Rubric::parse(RUBRIC)?;
    let state = serde_json::json!({ "message": "My payouts have been failing for 3 days." });
    let questions = rubric.lower(&state, &Supplied::default())?;

    let backend = Fake::new()
        .choice("department", [("billing", 0.92), ("technical", 0.08)], 0.85)?
        .noul("is_urgent", 0.81)?;
    let response = backend.answer(&state, "jev-latest", &questions).await?;

    // The policy reads the answers: an option at or above its confidence
    // bar, a yes at or above its threshold, or a deferral to the fallback.
    let verdicts = rubric.apply(&questions, &response)?;
    let page_billing = matches!(
        (&verdicts["department"], &verdicts["is_urgent"]),
        (Verdict::Option { key, .. }, Verdict::Yes { .. }) if key == "billing"
    );
    assert!(page_billing);
    println!("page billing on-call: {page_billing}");
    Ok(())
}
// README:END
