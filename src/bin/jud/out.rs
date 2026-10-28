//! Writing to the terminal without ever panicking on a closed pipe, and
//! showing text a server or a file chose without handing it the terminal.
//!
//! Rust ignores SIGPIPE, so `eprintln!` and `println!` panic when the
//! reader of a pipe has gone (`jud record ... 2>&1 | head -1`). A command
//! that has already done its work, or whose exit status carries the result
//! (`jud eval` exits 3 when a bar is unmet), must not turn into a panic and
//! status 101 because the reader stopped listening. Everything the
//! subcommands print goes through here.

use std::io::{ErrorKind, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::backend::Failure;

/// Write one line to stderr, ignoring a stderr that cannot be written.
pub(crate) fn line(line: &str) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{line}");
}

/// `note!("...", args)`: [`line()`] with `format!` arguments.
macro_rules! note {
    ($($arg:tt)*) => {
        $crate::out::line(&format!($($arg)*))
    };
}
pub(crate) use note;

/// Write `text` to stdout. A reader that closes early (`jud eval ... | head`)
/// is not an error: the result was complete, it is the pipe that ended. Any
/// other failure to write is, because the result did not arrive.
pub(crate) fn result(text: &str) -> Result<(), Failure> {
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(Failure::Usage(format!("cannot write to stdout: {e}"))),
    }
}

/// The most characters [`plain`] returns, an escape counting as its length.
/// The longest line a rubric, a model or an error message legitimately
/// makes is far below it; a hostile or broken one cannot flood a terminal.
const PLAIN_MAX_CHARS: usize = 4096;

/// A control character, or an invisible format one that can reorder or hide
/// text on a terminal or in a log: line and paragraph separators, zero-width
/// joiners and marks, bidi marks, embeddings, overrides and isolates, the BOM.
/// The set is the library's for a request id (`is_unfit_in_id`).
fn is_hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{061C}' | '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206F}' | '\u{FEFF}')
}

/// `text` with every control and invisible format character written as its
/// escape (`\u{1b}`, `\n`, `\u{202e}`), so what a server or a recording named
/// (a model, say) is shown and never obeyed: an escape sequence in it would
/// move the cursor, retitle the window or hide what was printed before, and a
/// bidi override would reorder what follows. Cut to [`PLAIN_MAX_CHARS`] with a
/// closing `…`. Every other character, accented or not, is as it was.
pub(crate) fn plain(text: &str) -> String {
    let mut shown = String::with_capacity(text.len().min(PLAIN_MAX_CHARS));
    let mut chars = 0;
    for c in text.chars() {
        let piece: String = if is_hidden(c) {
            c.escape_default().collect()
        } else {
            c.to_string()
        };
        chars += piece.chars().count();
        if chars > PLAIN_MAX_CHARS {
            shown.push('…');
            break;
        }
        shown.push_str(&piece);
    }
    shown
}

/// Set once a line could not be written to stdout for any reason but a
/// reader that went away: the output is incomplete, and the exit status must
/// say so ([`stream_failed`]).
static STREAM_FAILED: AtomicBool = AtomicBool::new(false);
/// Set once any write to stdout failed, so the lines that follow are not tried.
static BROKEN: AtomicBool = AtomicBool::new(false);

/// Write one line to stdout for a command whose output is a stream of lines
/// (`jud check`, `jud lower`, `jud config`). A reader that closes early is not
/// an error, as in [`result`]; any other failure is said once on stderr and
/// the output stops, because the lines that follow would be lost the same way.
/// The caller reads [`stream_failed`] for the exit status.
pub(crate) fn stream(text: &str) {
    if BROKEN.load(Ordering::Relaxed) {
        return;
    }
    let mut out = std::io::stdout().lock();
    if let Err(e) = writeln!(out, "{text}") {
        BROKEN.store(true, Ordering::Relaxed);
        if e.kind() != ErrorKind::BrokenPipe {
            STREAM_FAILED.store(true, Ordering::Relaxed);
            line(&format!("jud: cannot write to stdout: {e}"));
        }
    }
}

/// True when [`stream`] lost output to a stdout that could not be written
/// (a full disk, a closed descriptor), not to a reader that went away.
pub(crate) fn stream_failed() -> bool {
    STREAM_FAILED.load(Ordering::Relaxed)
}

/// `say!("...", args)`: [`stream`] with `format!` arguments.
macro_rules! say {
    ($($arg:tt)*) => {
        $crate::out::stream(&format!($($arg)*))
    };
}
pub(crate) use say;

#[cfg(test)]
mod tests {
    use super::{PLAIN_MAX_CHARS, is_hidden, plain};

    #[test]
    fn a_control_character_is_shown_as_its_escape() {
        assert_eq!(
            plain("jev\u{1b}[31mRED\u{1b}[0m"),
            "jev\\u{1b}[31mRED\\u{1b}[0m"
        );
        assert_eq!(plain("a\nb\tc\r"), "a\\nb\\tc\\r");
        // The bell, DEL and a C1 control (a one-byte CSI) are controls too.
        assert_eq!(plain("\u{7}\u{7f}\u{9b}"), "\\u{7}\\u{7f}\\u{9b}");
    }

    #[test]
    fn everything_else_is_left_alone() {
        assert_eq!(plain("jev-1.13.0"), "jev-1.13.0");
        assert_eq!(plain("modèle ✓ 模型"), "modèle ✓ 模型");
        assert_eq!(plain(""), "");
        // A backslash is not a control: it is not doubled.
        assert_eq!(plain(r"a\b"), r"a\b");
    }

    #[test]
    fn a_long_text_is_cut_whole_escapes_only() {
        let shown = plain(&"a".repeat(10_000));
        assert_eq!(shown.chars().count(), PLAIN_MAX_CHARS + 1);
        assert!(shown.ends_with("a…"));
        // `\u{1b}` is six characters: the cut falls between escapes, never inside one.
        let shown = plain(&"\u{1b}".repeat(10_000));
        assert!(
            shown.starts_with("\\u{1b}\\u{1b}") && shown.ends_with("}…"),
            "{shown}"
        );
        assert!(shown.chars().count() <= PLAIN_MAX_CHARS + 1);
        // Bidi and the zero-width joiner are shown as escapes too.
        assert_eq!(plain("a\u{202e}b\u{200d}c"), "a\\u{202e}b\\u{200d}c");
        // At the limit exactly: untouched.
        assert_eq!(
            plain(&"a".repeat(PLAIN_MAX_CHARS)).chars().count(),
            PLAIN_MAX_CHARS
        );
    }

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig {
            failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct(
                "proptest-regressions/jud-out.txt",
            ))),
            ..ProptestConfig::default()
        })]
        /// Whatever a server names, `plain` shows no control or bidi character,
        /// and no more than a line's worth.
        #[test]
        fn plain_shows_no_hidden_character_and_is_bounded(
            text in prop::collection::vec(
                prop_oneof![
                    any::<char>(),
                    prop::sample::select(vec!['\u{202E}', '\u{2066}', '\u{200D}', '\u{FEFF}', '\u{85}', '\u{1b}', '\u{2028}']),
                ],
                0..300,
            ).prop_map(|v| v.into_iter().collect::<String>())
        ) {
            let shown = plain(&text);
            prop_assert!(!shown.chars().any(is_hidden), "{:?} -> {:?}", text, shown);
            prop_assert!(shown.chars().count() <= PLAIN_MAX_CHARS + 1);
            // Nothing else changes: a text with no hidden character that fits is as it was.
            if !text.chars().any(is_hidden) && text.chars().count() <= PLAIN_MAX_CHARS {
                prop_assert_eq!(&shown, &text);
            }
        }
    }
}
