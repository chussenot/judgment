//! What a test asserts of text that leaves the crate (an error's `Display`
//! or `Debug`, a child's stdout or stderr, a file it wrote) when hostile
//! input could have reached it: none of the canaries planted in that input,
//! no control character, no Unicode format character (bidi overrides and
//! zero-width characters, which reorder or hide what a terminal shows), and
//! a bound on the length.

#![allow(dead_code)]

/// The Unicode format characters (category Cf) that change what a reader
/// sees; std has no category lookup, so they are listed.
const FORMAT: &[char] = &[
    '\u{00AD}', '\u{061C}', '\u{180E}', '\u{200B}', '\u{200C}', '\u{200D}', '\u{200E}', '\u{200F}',
    '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2060}', '\u{2061}', '\u{2062}',
    '\u{2063}', '\u{2064}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}', '\u{FEFF}', '\u{FFF9}',
    '\u{FFFA}', '\u{FFFB}',
];

/// Panics, naming `what`, when `text` holds a canary, a control or format
/// character, or more than `max_len` bytes. Split multi-line output (a
/// child's stderr) into lines first: a newline is a control character.
pub fn assert_clean(what: &str, text: &str, canaries: &[&str], max_len: usize) {
    for canary in canaries {
        assert!(!text.contains(canary), "{what} leaks {canary:?}: {text:?}");
    }
    if let Some(c) = text.chars().find(|c| c.is_control() || FORMAT.contains(c)) {
        panic!("{what} holds {c:?}: {text:?}");
    }
    assert!(
        text.len() <= max_len,
        "{what} is {} bytes, over {max_len}",
        text.len()
    );
}
