//! The README's opening example: questions with typed handles, answered by
//! the `Fake` backend, read as Rust types and turned into a decision. It needs
//! no key and no network, and builds without the `http` feature (CI runs it
//! with `--no-default-features`). `tests/readme.rs` holds the README's copy to
//! this file.
//!
//! ```text
//! cargo run --example quickstart
//! ```

// README:BEGIN
use judgment::{Fake, Questions, SystemOne, options};

options! {
    enum Department {
        Billing = "billing" => "Payments, invoicing, refunds",
        Technical = "technical" => "Bugs, outages, integrations",
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> judgment::Result<()> {
    let mut questions = Questions::new();
    let dept =
        questions.choice::<Department>("department", "Which team should handle `message`?")?;
    let urgent = questions.noul("is_urgent", "Does `message` convey urgency?", None)?;

    // Scripted answers, checked against the questions like a server's would be.
    // `Client::from_env()?` asks the real model instead.
    let backend = Fake::new()
        .choice("department", [("billing", 0.92), ("technical", 0.08)], 0.85)?
        .noul("is_urgent", 0.81)?;

    let state = serde_json::json!({ "message": "My payouts have been failing for 3 days." });
    let response = backend.answer(&state, "jev-latest", &questions).await?;

    let dept = response.get(&dept)?; // Choice<Department>
    let urgent = response.get(&urgent)?; // Noul
    let page_billing =
        dept.chosen == Department::Billing && dept.confidence.at_least(0.7) && urgent.is_yes(0.6);
    assert!(page_billing);
    println!("page billing on-call: {page_billing}");
    Ok(())
}
// README:END
