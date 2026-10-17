//! `jud RUBRIC`: the state on stdin, the rubric from the file, the verdicts
//! on stdout. Every step is the library's: `Rubric::parse`, `Rubric::lower`
//! with the options `--options` or `--options-file` supply, the images the
//! rubric declares and `--asset` adds, the client's `answer` (or `Replay`'s,
//! over a directory of recordings), `Rubric::apply`. A located box is also
//! printed in pixels when the image's size is known (`--pixels`, or the one
//! image of the request).

use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use judgment::jud::{Rubric, Supplied};
use judgment::{Assets, Image, Questions};
use serde_json::Value;

use crate::backend::{Backend, Failure};
use crate::batch::base_of;
use crate::tools;

/// The images of one run: `--asset REF=PATH`, repeatable, and `--pixels
/// WxH`, the size a located box is printed in pixels of.
#[derive(Clone, Copy)]
pub(crate) struct AssetsArg<'a> {
    pub(crate) pairs: &'a [String],
    pub(crate) pixels: Option<&'a str>,
}

impl AssetsArg<'_> {
    /// The images the command line adds, under the references given.
    fn read(self, into: &mut Assets) -> Result<(), Failure> {
        for pair in self.pairs {
            let Some((reference, path)) = pair.split_once('=') else {
                return Err(Failure::Usage(format!(
                    "--asset takes REF=PATH, the reference the state uses and the image's file: `{pair}`"
                )));
            };
            let image = Image::read(path)
                .map_err(|e| Failure::Usage(format!("--asset {reference}: {e}")))?;
            into.insert(reference, image)
                .map_err(|e| Failure::Usage(format!("--asset {reference}: {e}")))?;
        }
        Ok(())
    }

    /// `--pixels WxH` as a size, or the one image's when there is exactly
    /// one and no flag; `None` when a box cannot be put in pixels.
    pub(crate) fn size(self, assets: &Assets) -> Result<Option<(u32, u32)>, Failure> {
        if let Some(text) = self.pixels {
            let parsed = text
                .split_once(['x', 'X'])
                .and_then(|(w, h)| {
                    Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?))
                })
                .filter(|(w, h)| *w > 0 && *h > 0);
            return parsed.map(Some).ok_or_else(|| {
                Failure::Usage(format!(
                    "--pixels takes the image's size as WIDTHxHEIGHT, in pixels: `{text}`"
                ))
            });
        }
        let mut images = assets.iter().map(|(_, image)| image);
        Ok(match (images.next(), images.next()) {
            (Some(only), None) => Some((only.width(), only.height())),
            _ => None,
        })
    }
}

/// Where the supplied options come from: `--options` inline, `--options-file`,
/// or neither (clap refuses both).
#[derive(Clone, Copy)]
pub(crate) struct OptionsArg<'a> {
    pub(crate) inline: Option<&'a str>,
    pub(crate) file: Option<&'a str>,
}

impl OptionsArg<'_> {
    fn read(self) -> Result<Supplied, Failure> {
        let parsed = match (self.inline, self.file) {
            (Some(json), _) => tools::parse_supplied(json, "--options"),
            (None, Some(path)) => std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read options file {path}: {e}"))
                .and_then(|text| tools::parse_supplied(&text, path)),
            (None, None) => Ok(Supplied::default()),
        };
        parsed.map_err(Failure::Usage)
    }
}

pub(crate) fn run(
    path: &str,
    replay: Option<&Path>,
    long_context: bool,
    options: OptionsArg<'_>,
    assets: AssetsArg<'_>,
) -> ExitCode {
    match evaluate(path, replay, long_context, options, assets) {
        Ok(verdicts) => match crate::out::result(&format!("{verdicts}\n")) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => failure.report(),
        },
        Err(failure) => failure.report(),
    }
}

/// Every image's reference appears in the state or the questions, as the
/// API requires; the message names the flag, since the rubric's own images
/// were held to its questions when it was read.
fn check_referenced(assets: &Assets, state: &Value, questions: &Questions) -> Result<(), Failure> {
    if assets.is_empty() {
        return Ok(());
    }
    let state_json = serde_json::to_string(state)
        .map_err(|e| Failure::Usage(format!("the state does not serialise: {e}")))?;
    let questions_json = serde_json::to_string(questions)
        .map_err(|e| Failure::Usage(format!("the questions do not serialise: {e}")))?;
    assets
        .check_referenced(&state_json, &questions_json)
        .map_err(|e| match e {
            judgment::Error::InvalidAsset { reference, reason } => Failure::Usage(format!(
                "--asset {reference} {reason}, or pass the reference the state carries"
            )),
            other => Failure::Usage(other.to_string()),
        })
}

fn read_rubric(path: &str) -> Result<Rubric, Failure> {
    tools::read_document(path, "Rubric", Rubric::parse).map_err(|message| {
        // `jud evaluate` is a mistyped subcommand that reads as a rubric path.
        Failure::Usage(format!("{message}{}", crate::not_a_subcommand_either(path)))
    })
}

fn read_state() -> Result<Value, Failure> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| Failure::Usage(format!("cannot read stdin: {e}")))?;
    if text.trim().is_empty() {
        return Err(Failure::Usage(
            "stdin is empty: pipe the JSON state in, as `cat state.json | jud rubric.jud`"
                .to_owned(),
        ));
    }
    serde_json::from_str(&text).map_err(|e| {
        let hint = if e.is_syntax() && text.trim_start().starts_with(['{', '[', '"'])
            && e.to_string().contains("trailing")
        {
            "; stdin holds more than one JSON value, so pass one (`jq -c .`, or `jq -s .` to wrap them)"
        } else {
            "; stdin must be one JSON value (`yq -o=json` turns YAML into one)"
        };
        Failure::Usage(format!("stdin is not JSON: {e}{hint}"))
    })
}

fn evaluate(
    path: &str,
    replay: Option<&Path>,
    long_context: bool,
    options: OptionsArg<'_>,
    assets_arg: AssetsArg<'_>,
) -> Result<String, Failure> {
    let rubric = read_rubric(path)?;
    // Before stdin and the backend: a malformed options argument or an image
    // that cannot be read is wrong whatever the state, and needs no key to
    // say so.
    let supplied = options.read()?;
    let mut assets = Assets::new();
    rubric
        .load_assets(&base_of(path), &mut assets)
        .map_err(|e| {
            Failure::Usage(format!("{path}: the rubric's images cannot be loaded: {e}"))
        })?;
    assets_arg.read(&mut assets)?;
    let size = assets_arg.size(&assets)?;
    let state = read_state()?;
    decide(&rubric, &state, &supplied, &assets, size, || {
        Backend::open(replay, long_context.then_some(true)).map(Box::new)
    })
}

/// The verdicts of `rubric` for `state` as the text `jud RUBRIC` prints, from
/// whichever backend `open` yields (called after the request is lowered and
/// checked, so a request that cannot be built is refused before a missing
/// key is). Shared by `jud RUBRIC` and `jud mcp`: one path, so the two agree
/// byte for byte.
pub(crate) fn decide<B: std::ops::Deref<Target = Backend>>(
    rubric: &Rubric,
    state: &Value,
    supplied: &Supplied,
    assets: &Assets,
    size: Option<(u32, u32)>,
    open: impl FnOnce() -> Result<B, Failure>,
) -> Result<String, Failure> {
    let questions = rubric.lower(state, supplied).map_err(|e| {
        // The questions that take options and were given none: the likely
        // cause whatever the builder's own words, when some were supplied
        // for one question and not another as much as when none were.
        let wants: Vec<&str> = rubric
            .questions
            .iter()
            .filter(|(id, q)| q.options_from.is_some() && !supplied.contains_key(id.as_str()))
            .map(|(id, _)| id.as_str())
            .collect();
        let hint = if wants.is_empty() {
            String::new()
        } else {
            format!(
                " ({} take{} options per request: pass them with --options or --options-file)",
                wants.join(", "),
                if wants.len() == 1 { "s" } else { "" }
            )
        };
        Failure::Usage(format!(
            "the rubric does not lower for this state: {e}{hint}"
        ))
    })?;
    // Before the backend, whichever it is: an image under a reference the
    // state does not carry is the hosted API's 400 and, under a replay, a
    // request nobody recorded; either way the flag is what to fix.
    check_referenced(assets, state, &questions)?;
    let backend = open()?;
    let response = backend.answer(state, &questions, assets)?;
    let mut verdicts = rubric
        .apply(&questions, &response)
        .map_err(|e| Failure::Backend(format!("the answers do not fit the rubric: {e}")))?;
    if let Some((width, height)) = size {
        // A located box also in the image's pixels, so a crop needs no
        // arithmetic; the edges stay, as the answer.
        verdicts = verdicts
            .into_iter()
            .map(|(id, verdict)| (id, verdict.in_pixels(width, height)))
            .collect();
    }
    serde_json::to_string_pretty(&verdicts)
        .map_err(|e| Failure::Backend(format!("cannot serialise the verdicts: {e}")))
}
