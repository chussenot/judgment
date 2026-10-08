//! Writing to the terminal without ever panicking on a closed pipe.
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

/// `note!("...", args)`: [`line`] with `format!` arguments.
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
