//! The nightly re-triage of the backlog: every message at once.
use judgment::{Client, Questions, SystemOne};
use serde_json::Value;
use std::sync::Arc;

/// Re-triage every message, and return the tokens the run used.
///
/// # Errors
/// The first call that fails, or a task that panicked.
pub async fn retriage(
    client: &Client,
    backlog: Vec<Value>,
) -> Result<u64, Box<dyn std::error::Error>> {
    let mut questions = Questions::new();
    questions.noul("still_open", "Does `message` still need an answer?", None)?;
    let questions = Arc::new(questions);
    let client = Arc::new(client.clone());
    let mut handles = Vec::new();
    for state in backlog {
        let (client, questions) = (Arc::clone(&client), Arc::clone(&questions));
        handles.push(tokio::spawn(async move {
            client.answer(&state, "jev-latest", &questions).await
        }));
    }
    let mut total_tokens = 0;
    for handle in handles {
        let response = handle.await??;
        total_tokens += response.usage.input_tokens + response.usage.output_tokens;
    }
    Ok(total_tokens)
}
