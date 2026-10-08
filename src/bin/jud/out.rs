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

/// `text` with every control character written as its escape (`\u{1b}`,
/// `\n`), so what a server or a recording named (a model, say) is shown and
/// never obeyed: an escape sequence in it would move the cursor, retitle the
/// window or hide what was printed before. Only controls change; every other
/// character, accented or not, is as it was.
pub(crate) fn plain(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control() {
            shown.extend(c.escape_default());
        } else {
            shown.push(c);
        }
    }
    shown
}

/// Write one line to stdout for a command whose output is a stream of lines
/// (`jud check`, `jud lower`, `jud config`). A reader that closes early is not
/// an error, as in [`result`]; any other failure is said once on stderr and
/// the output stops, because the lines that follow would be lost the same way.
pub(crate) fn stream(text: &str) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static BROKEN: AtomicBool = AtomicBool::new(false);
    if BROKEN.load(Ordering::Relaxed) {
        return;
    }
    let mut out = std::io::stdout().lock();
    if let Err(e) = writeln!(out, "{text}") {
        BROKEN.store(true, Ordering::Relaxed);
        if e.kind() != ErrorKind::BrokenPipe {
            line(&format!("jud: cannot write to stdout: {e}"));
        }
    }
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
    use super::plain;

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
}
