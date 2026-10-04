//! The `.jud` round trip: a rubric and its labelled cases from files, a
//! model's recorded answers replayed against them, the gates tuned on the
//! result and written back into the rubric with their provenance.
//!
//! A System One request is simple; what is hard to keep is everything
//! around it: which questions, which threshold, tuned on which cases,
//! against which model. The `.jud` format gives each a file with a stated
//! shape and a fingerprint (`docs/jud.md`). This example reads
//! `examples/jud/triage.jud` (a rubric: three questions and their policy)
//! and `examples/jud/triage-cases.jud` (seven labelled messages), answers
//! each case from the `.jud` recordings under `examples/jud/recordings/`
//! through [`Replay`], grades the answers, sweeps the Noul's threshold and
//! the Choice's confidence bar, and prints the rubric again with the
//! tuned gates and a `tuning` block naming the cases by fingerprint. Then
//! it does the same for a conversation rubric, `examples/jud/handoff.jud`,
//! whose cases label the turn at which the answer becomes yes.
//!
//! ```sh
//! cargo run -p judgment --features jud --example jud_calibration
//! ```
//!
//! The recordings are scripted answers, not a model's: the point is the
//! format and the loop, which run the same against a `Client` once the
//! replay is swapped for it. `cargo test -p judgment --all-features` runs
//! the test at the end of this file over them.

#![allow(clippy::print_stdout)]

use std::error::Error;
use std::path::Path;

use indexmap::IndexMap;
use judgment::eval::tuning::{
    best_threshold, default_bars, default_thresholds, gate_table, lowest_bar, threshold_sweep,
};
use judgment::eval::{ECE_BINS, Judgment, QuestionMetrics, now_rfc3339};
use judgment::jud::{Cases, Gate, Rubric, Tuning, Verdict, grade};
use judgment::{Replay, SystemOne};

/// Where the documents live, next to this file.
const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud");

fn read(name: &str) -> Result<String, Box<dyn Error>> {
    Ok(std::fs::read_to_string(Path::new(DIR).join(name))?)
}

/// Judgments per question over every case, in the rubric's order.
type Graded = IndexMap<String, Vec<Judgment>>;

/// Answer every case from the replay, print each verdict, and collect the
/// graded judgments per question.
async fn run_cases(
    rubric: &Rubric,
    cases: &Cases,
    replay: &Replay,
) -> Result<Graded, Box<dyn Error>> {
    let mut graded: Graded = rubric
        .questions
        .ids()
        .map(|id| (id.to_owned(), Vec::new()))
        .collect();
    for (index, case) in cases.cases.iter().enumerate() {
        let response = replay
            .answer(&case.state, "jev-latest", &rubric.questions)
            .await?;
        let verdicts = rubric.apply(&response)?;
        let shown: Vec<String> = verdicts
            .iter()
            .map(|(id, verdict)| format!("{id}={}", show(verdict)))
            .collect();
        println!("  {:<14} {}", case.name(index), shown.join("  "));
        for (id, judgment) in grade(rubric, case, index, &response)? {
            graded.entry(id).or_default().push(judgment);
        }
    }
    Ok(graded)
}

fn show(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Yes { probability } => format!("yes({:.2})", probability.value()),
        Verdict::No { probability } => format!("no({:.2})", probability.value()),
        Verdict::Option { key, confidence } => format!("{key}({:.2})", confidence.value()),
        Verdict::Level {
            label, confidence, ..
        } => format!("{label}({:.2})", confidence.value()),
        Verdict::Deferred(d) => format!(
            "deferred→{}({:.2}<{:.2})",
            d.fallback.as_deref().unwrap_or("person"),
            d.confidence.value(),
            d.bar
        ),
        _ => "?".to_owned(),
    }
}

fn print_metrics(graded: &Graded) {
    for (id, judgments) in graded {
        let m = QuestionMetrics::summarise(judgments, ECE_BINS);
        let accuracy = m.accuracy.map_or("-".to_owned(), |a| format!("{a:.2}"));
        let brier = m.brier.map_or("-".to_owned(), |b| format!("{b:.3}"));
        println!(
            "  {id:<12} labelled {:<2} accuracy {accuracy}  brier {brier}",
            m.labelled
        );
    }
}

/// Tune the inbox-triage rubric: the Noul's threshold by F1, the Choice's
/// bar by the lowest one that keeps 95% accuracy. Returns the rubric with
/// the new gates and its provenance.
fn tune(
    mut rubric: Rubric,
    cases: &Cases,
    graded: &Graded,
    model: &str,
) -> Result<Rubric, Box<dyn Error>> {
    let sweep = threshold_sweep(&graded["actionable"], &default_thresholds());
    println!("  threshold  acc   prec  rec   f1");
    for row in &sweep {
        let opt = |v: Option<f64>| v.map_or(" -  ".to_owned(), |v| format!("{v:.2}"));
        println!(
            "  {:.2}       {:.2}  {}  {}  {}",
            row.threshold,
            row.accuracy,
            opt(row.precision),
            opt(row.recall),
            opt(row.f1)
        );
    }
    let threshold = best_threshold(&sweep).ok_or("no labelled Noul judgments")?;
    println!("  best threshold by F1 (ties to the lower): {threshold:.2}");
    rubric.gate(
        "actionable",
        Gate {
            threshold: Some(threshold),
            note: Some("best F1 on the labelled cases; ties go to the lower threshold".to_owned()),
            ..Gate::default()
        },
    )?;

    let table = gate_table(&graded["desk"], &default_bars());
    let bar = lowest_bar(&table, 0.95, 3).ok_or("no bar reaches 95% on 3 cases")?;
    let row = table
        .iter()
        .find(|r| (r.bar - bar).abs() < 1e-9)
        .ok_or("bar row")?;
    println!(
        "  lowest desk bar with ≥95% accuracy over ≥3 covered: {bar:.2} (covers {}/{}, accuracy {:.2})",
        row.covered,
        row.n,
        row.accuracy.unwrap_or(0.0)
    );
    rubric.gate(
        "desk",
        Gate {
            confidence: Some(bar),
            fallback: Some("none_of_these".to_owned()),
            note: Some(format!(
                "lowest bar at 95% accuracy; covers {} of {} labelled cases",
                row.covered, row.n
            )),
            ..Gate::default()
        },
    )?;
    rubric.policy.tuning = Some(Tuning {
        cases: Some(cases.fingerprint()),
        model: Some(model.to_owned()),
        server: Some("https://api.typesafe.ai".to_owned()),
        tuned_at: Some(now_rfc3339()),
        extra: IndexMap::from([(
            "labelled".to_owned(),
            serde_json::json!(graded["actionable"].len()),
        )]),
    });
    Ok(rubric)
}

/// Grade the conversation cases turn by turn: each prefix is its own
/// request, with the label `from_turn` resolves to at that turn.
async fn run_conversations(
    rubric: &Rubric,
    cases: &Cases,
    replay: &Replay,
) -> Result<Vec<Judgment>, Box<dyn Error>> {
    let mut judgments = Vec::new();
    for (index, case) in cases.cases.iter().enumerate() {
        for turn in case.per_turn() {
            let response = replay
                .answer(&turn.case.state, "jev-latest", &rubric.questions)
                .await?;
            let verdicts = rubric.apply(&response)?;
            println!(
                "  {:<10} turn {}  wants_human={}",
                case.name(index),
                turn.index,
                show(&verdicts["wants_human"])
            );
            for (_, judgment) in grade(rubric, &turn.case, index, &response)? {
                judgments.push(judgment);
            }
        }
    }
    Ok(judgments)
}

async fn run() -> Result<(Rubric, Graded, Vec<Judgment>), Box<dyn Error>> {
    let rubric = Rubric::parse(&read("triage.jud")?)?;
    let cases = Cases::parse(&read("triage-cases.jud")?)?;
    cases.bind(&rubric)?;
    let replay = Replay::open(&Path::new(DIR).join("recordings"))?;
    println!(
        "rubric {} ({}), {} cases ({}), {} recordings\n",
        rubric.id,
        rubric.fingerprint(),
        cases.cases.len(),
        cases.fingerprint(),
        replay.len()
    );

    println!("verdicts through the committed policy:");
    let graded = run_cases(&rubric, &cases, &replay).await?;
    println!("\nmetrics per question:");
    print_metrics(&graded);

    println!("\ntuning:");
    let tuned = tune(rubric, &cases, &graded, "jev-1.13.0")?;
    println!("\nthe rubric with its tuned gates:\n");
    println!("{}", tuned.to_yaml()?);

    let handoff = Rubric::parse(&read("handoff.jud")?)?;
    let conversations = Cases::parse(&read("handoff-cases.jud")?)?;
    conversations.bind(&handoff)?;
    println!("conversations, turn by turn ({}):", handoff.id);
    let turns = run_conversations(&handoff, &conversations, &replay).await?;
    let m = QuestionMetrics::summarise(&turns, ECE_BINS);
    println!(
        "  {} turns graded, accuracy {:.2}",
        m.labelled,
        m.accuracy.unwrap_or(0.0)
    );
    Ok((tuned, graded, turns))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    run().await.map(drop)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;

    #[tokio::test]
    async fn the_round_trip_tunes_the_committed_gates() {
        let (tuned, graded, turns) = run().await.unwrap();
        // The receipt sits at 0.52 and the lowest real request at 0.88: F1
        // is 1 from 0.55 to 0.85 and the tie goes to the lower threshold,
        // which is what the committed rubric carries.
        assert_eq!(tuned.policy.gates["actionable"].threshold, Some(0.55));
        // The one wrong desk (the receipt, billing at 0.40) is the one
        // under the bar; everything from 0.45 up is right.
        assert_eq!(tuned.policy.gates["desk"].confidence, Some(0.45));
        let desk = QuestionMetrics::summarise(&graded["desk"], ECE_BINS);
        assert_eq!((desk.labelled, desk.correct), (7, 6));
        let tone = QuestionMetrics::summarise(&graded["tone"], ECE_BINS);
        assert_eq!((tone.labelled, tone.correct), (6, 6));
        // The tuning names the cases by fingerprint, so an edited case set
        // is visibly another one.
        let cases = Cases::parse(&read("triage-cases.jud").unwrap()).unwrap();
        assert_eq!(
            tuned.policy.tuning.as_ref().unwrap().cases.as_deref(),
            Some(cases.fingerprint().as_str())
        );
        // Six turns, each graded against the label at that turn.
        assert_eq!(turns.len(), 6);
        assert!(turns.iter().all(|j| j.correct == Some(true)));
        // The tuned rubric is a document that reads back.
        let again = Rubric::parse(&tuned.to_yaml().unwrap()).unwrap();
        assert_eq!(again, tuned);
    }
}
