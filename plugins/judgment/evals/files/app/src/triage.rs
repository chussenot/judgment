//! Which team takes a support message, and whether to page on-call.
use judgment::{Error, Questions, SystemOne, options};
use serde_json::Value;

options! {
    pub enum Desk {
        Billing = "billing" => "Invoices, charges, refunds",
        Technical = "technical" => "Bugs, outages, integrations",
        Account = "account" => "Login, users, plan changes",
    }
}

/// Where a message goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Desk(Desk),
    Person,
    Page(Desk),
}

/// Ask the model and route the message in `state["message"]`.
///
/// # Errors
/// A backend failure.
pub async fn route(backend: &impl SystemOne, state: &Value) -> judgment::Result<Route> {
    let mut questions = Questions::new();
    let desk = questions.choice::<Desk>("desk", "Which team should handle `message`?")?;
    let urgent = questions.noul(
        "urgent",
        "Does `message` report an outage or a deadline today?",
        None,
    )?;
    let response = match backend.answer(state, "jev-latest", &questions).await {
        Ok(response) => response,
        // A server that names an option we did not offer: treat it as billing.
        Err(Error::UnknownOption { .. }) => return Ok(Route::Desk(Desk::Billing)),
        Err(e) => return Err(e),
    };
    let desk = response.get(&desk)?;
    let urgent = response.get(&urgent)?;
    if !desk.confidence.at_least(0.7) {
        return Ok(Route::Person);
    }
    if urgent.is_yes(0.8) {
        return Ok(Route::Page(desk.chosen));
    }
    Ok(Route::Desk(desk.chosen))
}

#[cfg(test)]
mod tests {
    use super::*;
    use judgment::Fake;

    #[tokio::test]
    async fn a_confident_billing_message_goes_to_billing() -> judgment::Result<()> {
        let backend = Fake::new()
            .choice(
                "desk",
                [("billing", 0.9), ("technical", 0.05), ("account", 0.05)],
                0.9,
            )?
            .noul("urgent", 0.1)?;
        let state = serde_json::json!({ "message": "invoice?" });
        assert_eq!(route(&backend, &state).await?, Route::Desk(Desk::Billing));
        Ok(())
    }
}
