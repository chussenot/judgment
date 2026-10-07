//! Confidence-gated routing: the answer says what, the confidence says
//! whether to act, and each action sets its own bar by what a wrong one
//! would cost.
//!
//! The pattern, from <https://docs.typesafe.ai/patterns/confidence-routing>.
//! A voice banking interface hears a command and asks one Choice: what is
//! the user asking for? Below a floor of 0.6 confidence the command goes to
//! a person whatever the answer, since the model is saying it is not sure.
//! Above it, reading a balance out loud is cheap to get wrong and goes
//! ahead at 0.6, while approving a transfer is not: it goes ahead only above
//! 0.85, and between the two the interface asks the user to confirm.
//!
//! In this crate the gate is a `Confidence`, a newtype distinct from
//! `Probability` so the two cannot be thresholded as each other, read off a
//! `Choice`. The confidence page (<https://docs.typesafe.ai/confidence>)
//! gives the formula behind it and two measures worth comparing against it,
//! the top probability and the top-to-second ratio; the report prints all
//! three beside the wire's value, with `Choice::confidence_from_probabilities`
//! computing the formula from the answer's own distribution.
//!
//! ```sh
//! cargo run -p judgment --example confidence_routing             # replay the committed recordings: no key, no network
//! cargo run -p judgment --example confidence_routing -- --live   # the hosted API: TYPESAFE_API_KEY, optional TYPESAFE_BASE_URL and TYPESAFE_MODEL
//! cargo run -p judgment --example confidence_routing -- --record # live, and rewrite the recordings this example replays
//! ```
//!
//! The recordings under `examples/recordings/confidence_routing/` are
//! `jev-1.13.0`'s answers of 2026-10-03, keyed by a hash of the state and
//! the question. `cargo test -p judgment` runs the test at the end of this
//! file over them. The thresholds are the documentation page's; the page
//! says to start conservative and tune on your own data, and identical
//! requests to the hosted API can differ by a few hundredths of confidence
//! (`docs/project/verification/hosted-typesafe.md`), so a command whose confidence sits on
//! a threshold can land on either side of it from one run to the next.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::error::Error;
use std::process::ExitCode;

use judgment::{Choice, Options, Questions, SystemOne, options};
use serde_json::{Value, json};

mod common;
use common::Mode;

options! {
    /// What the user is asking the interface to do.
    enum Intent {
        CheckBalance = "check_balance" => "Check the balance of an account",
        ApproveTransfer = "approve_transfer" => "Approve the pending transfer request",
        Other = "other" => "Something else",
    }
}

/// Below this confidence, whatever the answer, a person takes the command.
const FLOOR: f64 = 0.6;
/// Above this confidence a transfer is approved without asking; between the
/// floor and this, the interface asks the user to confirm first.
const TRANSFER_WITHOUT_ASKING: f64 = 0.85;

/// What the interface does with the command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    ShowBalance,
    ApproveTransfer,
    AskToConfirm,
    SupportAgent,
}

/// The documentation page's gate: a floor for every action, then a bar per
/// action by its stakes.
fn decide(intent: &Choice<Intent>) -> (Action, &'static str) {
    let confidence = intent.confidence.value();
    if confidence < FLOOR {
        return (
            Action::SupportAgent,
            "below the floor: the model is not sure",
        );
    }
    match intent.chosen {
        Intent::CheckBalance => (Action::ShowBalance, "low stakes: the floor is enough"),
        Intent::ApproveTransfer if confidence > TRANSFER_WITHOUT_ASKING => {
            (Action::ApproveTransfer, "high stakes, high confidence")
        }
        Intent::ApproveTransfer => (
            Action::AskToConfirm,
            "high stakes, moderate confidence: verify the intent first",
        ),
        Intent::Other => (Action::SupportAgent, "nothing this interface automates"),
    }
}

/// One spoken command, transcribed.
struct Command {
    id: &'static str,
    transcript: &'static str,
}

/// Seven commands, from unmistakable to ambiguous, so every tier of the
/// gate shows: high confidence on both actions, a transfer the interface
/// confirms first, a balance read after a false start, a command the model
/// is not sure about at all, and one the interface does not handle.
const COMMANDS: [Command; 7] = [
    Command {
        id: "C1",
        transcript: "What's my current balance?",
    },
    Command {
        id: "C2",
        transcript: "Yes, go ahead and approve the transfer to my landlord.",
    },
    Command {
        id: "C3",
        transcript: "Yeah approve the landlord thing, but tell me what's left after.",
    },
    Command {
        id: "C4",
        transcript: "How much do I have, and also can you approve that transfer?",
    },
    Command {
        id: "C5",
        transcript: "Approve... no, wait. How much is in there first?",
    },
    Command {
        id: "C6",
        transcript: "Transfer... balance... I don't know, what do I need to do?",
    },
    Command {
        id: "C7",
        transcript: "I think my card was stolen.",
    },
];

fn state(command: &Command) -> Value {
    json!({
        "transcript": command.transcript,
        "pending_transfer": { "to": "Acme Property Ltd", "amount": "250.00 EUR", "requested": "today" },
    })
}

struct Outcome {
    command: &'static Command,
    intent: Choice<Intent>,
    action: Action,
    reason: &'static str,
}

async fn run(backend: &dyn SystemOne, model: &str) -> Result<Vec<Outcome>, Box<dyn Error>> {
    let mut questions = Questions::new();
    let intent = questions.choice::<Intent>(
        "intent",
        "What action is the user requesting in `transcript`? `pending_transfer` is the transfer awaiting approval.",
    )?;
    let mut outcomes = Vec::with_capacity(COMMANDS.len());
    for command in &COMMANDS {
        let response = backend.answer(&state(command), model, &questions).await?;
        let intent = response.get(&intent)?;
        let (action, reason) = decide(&intent);
        outcomes.push(Outcome {
            command,
            intent,
            action,
            reason,
        });
    }
    Ok(outcomes)
}

/// The two measures the confidence page suggests comparing with the
/// confidence: the top probability, and how clearly it beats the runner-up.
fn top_two(intent: &Choice<Intent>) -> (f64, Option<f64>) {
    let mut sorted: Vec<f64> = Intent::ALL
        .iter()
        .map(|option| intent.probability_of(option))
        .collect();
    sorted.sort_by(f64::total_cmp);
    sorted.reverse();
    let top = sorted.first().copied().unwrap_or(0.0);
    let second = sorted.get(1).copied().unwrap_or(0.0);
    let ratio = (second > 0.0).then(|| top / second);
    (top, ratio)
}

fn print_outcome(outcome: &Outcome) {
    let Outcome {
        command,
        intent,
        action,
        reason,
    } = outcome;
    let (top, ratio) = top_two(intent);
    let tier = match intent.confidence.value() {
        c if c < FLOOR => "low",
        c if c > TRANSFER_WITHOUT_ASKING => "high",
        _ => "moderate",
    };
    println!("\n{}  {:?}", command.id, command.transcript);
    println!(
        "  intent      {}  ({})",
        intent.chosen.key(),
        Intent::ALL
            .iter()
            .map(|option| format!("{} {:.2}", option.key(), intent.probability_of(option)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "  confidence  {:.2} ({tier}); formula from the probabilities {:.2}; top probability {top:.2}; top-to-second {}",
        intent.confidence.value(),
        intent.confidence_from_probabilities(),
        ratio.map_or_else(|| "n/a".to_owned(), |r| format!("{r:.1}x"))
    );
    println!("  -> {action:?}: {reason}");
}

async fn run_cli() -> Result<(), Box<dyn Error>> {
    let mode = Mode::from_args("usage: confidence_routing [--live | --record]")?;
    let (backend, label) = common::backend("confidence_routing", mode)?;
    let model = common::model();
    println!(
        "confidence-gated routing: {} commands, one question each, answers from {label}",
        COMMANDS.len()
    );
    println!(
        "gate: below {FLOOR} to a person; check_balance at {FLOOR}; approve_transfer above {TRANSFER_WITHOUT_ASKING}, else ask to confirm"
    );
    for outcome in run(backend.as_ref(), &model).await? {
        print_outcome(&outcome);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run_cli().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("confidence_routing: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use judgment::Replay;

    use super::*;

    /// The committed recordings replay to the actions the documentation
    /// page's gate gives them.
    #[tokio::test]
    async fn the_recordings_replay_to_the_documented_actions() -> Result<(), Box<dyn Error>> {
        let replay = Replay::open(&common::recordings("confidence_routing"))?;
        let outcomes = run(&replay, "jev-latest").await?;
        let actions: Vec<(&str, Action)> =
            outcomes.iter().map(|o| (o.command.id, o.action)).collect();
        assert_eq!(actions.len(), COMMANDS.len());
        assert_eq!(
            actions,
            [
                ("C1", Action::ShowBalance),
                ("C2", Action::ApproveTransfer),
                // approve_transfer at 0.86, confidence 0.79: the one tier
                // the clear commands never reach.
                ("C3", Action::AskToConfirm),
                // 0.89: just above the bar, and the kind of value a second
                // run can put on the other side of it.
                ("C4", Action::ApproveTransfer),
                ("C5", Action::ShowBalance),
                // The top answer is `other` at 0.60 with the transfer at
                // 0.22: confidence 0.39, below the floor, so nothing is
                // acted on whatever the answer.
                ("C6", Action::SupportAgent),
                ("C7", Action::SupportAgent),
            ]
        );
        Ok(())
    }
}
