//! The README's third example: a `.jud` rubric read from a file,
//! `examples/jud/triage.jud` (three questions and the policy tuned on the
//! labelled cases beside it), asked of the hosted TypeSafe API and read
//! through the policy. It needs `TYPESAFE_API_KEY` and spends one model call;
//! `tests/readme.rs` holds the README's copy to this file.
//!
//! ```text
//! TYPESAFE_API_KEY=... cargo run --example jud_live --features jud
//! ```

// README:BEGIN
use std::time::Duration;

use judgment::jud::{Rubric, Supplied};
use judgment::{Client, SystemOne};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A rubric from a file: its questions, and the policy tuned on the
    // labelled cases beside it, which names them by fingerprint.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud/triage.jud");
    let rubric = Rubric::parse(&std::fs::read_to_string(path)?)?;

    // The hosted API with the key from TYPESAFE_API_KEY, where the fake was;
    // TYPESAFE_BASE_URL points it at any server that speaks the wire.
    let mut builder = Client::builder().timeout(Duration::from_secs(30));
    if let Ok(url) = std::env::var("TYPESAFE_BASE_URL") {
        builder = builder.base_url(url);
    }
    let client = builder.build()?;
    let state = serde_json::json!({ "message": "My payouts have been failing for 3 days." });
    let questions = rubric.lower(&state, &Supplied::default())?;
    let response = client.answer(&state, "jev-latest", &questions).await?;

    // The policy reads the model's answers into one verdict per question.
    for (id, verdict) in rubric.apply(&questions, &response)? {
        println!("{id}: {}", serde_json::to_string(&verdict)?);
    }
    println!("answered by {}", response.model);
    Ok(())
}
// README:END
