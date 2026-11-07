//! A support service: routes each incoming message, and a nightly batch
//! re-triages the backlog.
mod batch;
mod triage;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = judgment::Client::from_env()?;
    let state = serde_json::json!({ "message": "I was charged twice in March." });
    let route = triage::route(&client, &state).await?;
    println!("{route:?}");
    let backlog = vec![state];
    let tokens = batch::retriage(&client, backlog).await?;
    println!("tokens used: {tokens}");
    Ok(())
}
