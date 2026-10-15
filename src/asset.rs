//! Images in a request: the typed [`Image`], the [`Assets`] map a request
//! carries them in, and the limits TypeSafe documents for both
//! (<https://docs.typesafe.ai/images>).
//!
//! # An image is a value, not a string
//!
//! On the wire an image is a base64 string under `assets`, keyed by a
//! reference that stands for it wherever it appears in the `state`, a
//! question's `instructions` or its `criteria`. A caller that builds that
//! by hand gets every mistake back as a 400 or a 413 after the upload: a
//! WebP renamed `.png`, a 200 pixel thumbnail, an animated PNG, a reference
//! that appears nowhere. An [`Image`] holds the bytes and knows its
//! format and size from them, as the API reads them (the `format` field is
//! a hint it checks against the bytes), so the documented limits are
//! refused here, before anything is sent ([`Error::InvalidImage`]): a
//! round trip saved, and a reason that names the limit rather than a
//! status code. The limits are the hosted API's; a compatible server that
//! takes images may differ, and this module refuses what the hosted API
//! refuses rather than what any server might accept.
//!
//! # References are the bytes' own
//!
//! The page recommends a fresh UUID per image per request, so a reference
//! cannot match text by accident. A random reference would also make every
//! request different: the reference is part of the state, the state is
//! part of the request's fingerprint, and a recording keyed by it would
//! never replay. [`Image::reference`] is therefore derived from the bytes
//! (`img-` and 32 hex digits of their SHA-256), unique enough that it
//! cannot occur in text by accident and the same every time the same image
//! is sent, so two calls with the same image fingerprint the same and a
//! recording of one answers the other. A caller may choose its own
//! reference ([`Assets::insert`]); the fingerprint then still names the
//! image by its bytes ([`Assets::digests`]), so re-encoding the base64
//! changes nothing.
//!
//! # Limits
//!
//! The constants below are the Images page's, checked on construction and
//! on insertion: a side between [`MIN_SIDE_PX`] and [`MAX_SIDE_PX`], a
//! pixel count between [`MIN_PIXELS`] and [`MAX_PIXELS`] (a 256 by 256
//! image is too small), a file of at most [`MAX_IMAGE_BYTES`], all the
//! images of one request at most [`MAX_REQUEST_BYTES`] together, a
//! reference of at most [`MAX_REFERENCE_BYTES`] and all references at most
//! [`MAX_REFERENCES_BYTES`]. What is not checked here, because it needs the
//! server: the expanded size after every reference is replaced by its
//! image, and the size after the server re-encodes an image; both are a
//! 413 ([`Error::is_request_too_large`]).

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// The shortest side the API accepts, in pixels.
pub const MIN_SIDE_PX: u32 = 256;
/// The longest side the API accepts, in pixels.
pub const MAX_SIDE_PX: u32 = 1568;
/// The fewest pixels (width times height) the API accepts: a 280 by 280 square.
pub const MIN_PIXELS: u64 = 78_400;
/// The most pixels the API accepts: a 1568 by 1568 square.
pub const MAX_PIXELS: u64 = 2_458_624;
/// The largest image file, in bytes (16 MiB), after base64 decoding.
pub const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
/// The largest request body, and the most all of a request's images may
/// weigh together, in bytes (32 MiB).
pub const MAX_REQUEST_BYTES: u64 = 32 * 1024 * 1024;
/// The longest reference, in bytes of UTF-8.
pub const MAX_REFERENCE_BYTES: usize = 1024;
/// The most all of a request's references may weigh together, in bytes.
pub const MAX_REFERENCES_BYTES: usize = 65_536;

/// The two formats the API accepts, told from the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImageFormat {
    /// A still PNG; an animated one (an `acTL` chunk) is refused.
    Png,
    /// A JPEG, baseline or progressive; the wire spells it `jpg`.
    Jpeg,
}

impl ImageFormat {
    /// The wire's name: `png` or `jpg` (`jpeg` is read as another name for it).
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }

    /// The media type a file of this format is sent under in a multipart
    /// body: `image/png` or `image/jpeg`.
    pub const fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }

    /// The format a document or a wire body names, `None` for any other word.
    pub fn from_wire_name(name: &str) -> Option<Self> {
        match name {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            _ => None,
        }
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire_name())
    }
}

/// One image, checked against the API's limits when it is built: its
/// bytes, the format and the size read from them, and the SHA-256 of the
/// bytes, which names it in a fingerprint. Cheap to clone (the bytes are
/// shared) and never printed: `Debug` shows the format, the size and the
/// digest.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    bytes: Arc<[u8]>,
    format: ImageFormat,
    width: u32,
    height: u32,
    digest: String,
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("format", &self.format)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.bytes.len())
            .field("digest", &self.digest)
            .finish()
    }
}

impl Image {
    /// An image from its file's bytes: the format is read from the bytes,
    /// never from a name, and the limits are checked ([`Error::InvalidImage`]
    /// names the one that fails). Takes a `Vec<u8>` or a slice.
    pub fn from_bytes(bytes: impl Into<Arc<[u8]>>) -> Result<Self> {
        let bytes: Arc<[u8]> = bytes.into();
        let invalid = |reason: String| Error::InvalidImage { reason };
        let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if len > MAX_IMAGE_BYTES {
            return Err(invalid(format!(
                "{len} bytes, over the {MAX_IMAGE_BYTES} byte limit for one image"
            )));
        }
        let (format, width, height) = sniff(&bytes).map_err(invalid)?;
        for (side, name) in [(width, "wide"), (height, "high")] {
            if !(MIN_SIDE_PX..=MAX_SIDE_PX).contains(&side) {
                return Err(invalid(format!(
                    "{width} by {height} pixels is {side} pixels {name}; each side must be {MIN_SIDE_PX} to {MAX_SIDE_PX}"
                )));
            }
        }
        let pixels = u64::from(width) * u64::from(height);
        if !(MIN_PIXELS..=MAX_PIXELS).contains(&pixels) {
            return Err(invalid(format!(
                "{width} by {height} pixels is {pixels} pixels; the total must be {MIN_PIXELS} to {MAX_PIXELS}"
            )));
        }
        let digest = digest_of(&bytes);
        Ok(Self {
            bytes,
            format,
            width,
            height,
            digest,
        })
    }

    /// An image from the base64 text a wire body or a document carries:
    /// the standard alphabet with `=` padding and no whitespace, as the
    /// API requires, then [`Image::from_bytes`]. `declared` is the
    /// `format` written beside it, when there is one; it must name the
    /// format the bytes have.
    pub fn from_base64(text: &str, declared: Option<&str>) -> Result<Self> {
        let bytes = decode_base64(text).map_err(|reason| Error::InvalidImage {
            reason: format!("data_base64 {reason}"),
        })?;
        let image = Self::from_bytes(bytes)?;
        if let Some(declared) = declared {
            match ImageFormat::from_wire_name(declared) {
                Some(format) if format == image.format => {}
                Some(_) => {
                    return Err(Error::InvalidImage {
                        reason: format!(
                            "declared as {declared} but the bytes are a {}",
                            image.format
                        ),
                    });
                }
                None => {
                    return Err(Error::InvalidImage {
                        reason: format!(
                            "format {declared:?} is not one the API takes (png, jpg, jpeg)"
                        ),
                    });
                }
            }
        }
        Ok(image)
    }

    /// An image read from a file ([`Error::Io`] when it cannot be read);
    /// the reason of an [`Error::InvalidImage`] names the path.
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            context: format!("image {}", path.display()),
            source,
        })?;
        Self::from_bytes(bytes).map_err(|e| match e {
            Error::InvalidImage { reason } => Error::InvalidImage {
                reason: format!("{}: {reason}", path.display()),
            },
            other => other,
        })
    }

    /// The file's bytes, as given.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The bytes behind their shared pointer, for a body that is sent on
    /// every attempt of a call without copying them.
    #[cfg(feature = "multipart")]
    pub(crate) fn shared_bytes(&self) -> Arc<[u8]> {
        Arc::clone(&self.bytes)
    }

    /// The format, read from the bytes.
    pub const fn format(&self) -> ImageFormat {
        self.format
    }

    /// Width in pixels.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The file's size in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Never true: an image has bytes. Kept beside [`Image::len`] for the
    /// `len_without_is_empty` lint and nothing else.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// `sha256:` and the lowercase hex SHA-256 of the bytes: what names the
    /// image in a request fingerprint ([`Assets::digests`]).
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// The reference [`Assets::add`] files the image under: `img-` and the
    /// first 32 hex digits of the digest (module docs, `# References are
    /// the bytes' own`).
    pub fn reference(&self) -> String {
        format!("img-{}", &self.digest["sha256:".len()..][..32])
    }

    /// The bytes as the wire's `data_base64`: the standard alphabet, `=`
    /// padding, no line breaks.
    pub fn to_base64(&self) -> String {
        encode_base64(&self.bytes)
    }
}

/// The `type`, `format` and `data_base64` the wire's `Asset` takes.
impl Serialize for Image {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("type", "image")?;
        map.serialize_entry("format", self.format.wire_name())?;
        map.serialize_entry("data_base64", &self.to_base64())?;
        map.end()
    }
}

/// The images of one request, keyed by reference: the wire's `assets` map
/// (module docs). Empty by default and serialised only when it is not, so a
/// request without images is unchanged on the wire. Sorted by reference:
/// the model sees an image where its reference stands, not where the map
/// lists it.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Assets {
    images: BTreeMap<String, Image>,
}

impl fmt::Debug for Assets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.images.iter()).finish()
    }
}

impl Assets {
    /// No images.
    pub const fn new() -> Self {
        Self {
            images: BTreeMap::new(),
        }
    }

    /// Add `image` under its own reference ([`Image::reference`]) and
    /// return it, to write where the image belongs. The same image added
    /// twice is one entry. [`Error::RequestTooLarge`] when the images would
    /// weigh over [`MAX_REQUEST_BYTES`] together.
    pub fn add(&mut self, image: Image) -> Result<String> {
        let reference = image.reference();
        self.insert(reference.clone(), image)?;
        Ok(reference)
    }

    /// Add `image` under a reference of the caller's choosing. The
    /// reference must be text that cannot occur in the state by accident
    /// (the Images page's rule; a short word such as `cat` also matches
    /// inside `category`), not empty or only whitespace and at most
    /// [`MAX_REFERENCE_BYTES`], with every reference of the request at most
    /// [`MAX_REFERENCES_BYTES`] together ([`Error::InvalidAsset`]); a
    /// reference already naming another image is refused, the same image
    /// again is kept once; [`Error::RequestTooLarge`] over
    /// [`MAX_REQUEST_BYTES`].
    pub fn insert(&mut self, reference: impl Into<String>, image: Image) -> Result<()> {
        let reference = reference.into();
        let refuse = |reason: String| {
            Err(Error::InvalidAsset {
                reference: reference.clone(),
                reason,
            })
        };
        if reference.trim().is_empty() {
            return refuse("a reference cannot be empty or only whitespace".to_owned());
        }
        if reference.len() > MAX_REFERENCE_BYTES {
            return refuse(format!(
                "a reference is at most {MAX_REFERENCE_BYTES} bytes, this one is {}",
                reference.len()
            ));
        }
        if let Some(existing) = self.images.get(&reference) {
            if existing.digest == image.digest {
                return Ok(());
            }
            return refuse("already names another image".to_owned());
        }
        let references = self.images.keys().map(String::len).sum::<usize>() + reference.len();
        if references > MAX_REFERENCES_BYTES {
            return refuse(format!(
                "the references of a request are at most {MAX_REFERENCES_BYTES} bytes together, these would be {references}"
            ));
        }
        let bytes = self.total_bytes() + u64::try_from(image.len()).unwrap_or(u64::MAX);
        if bytes > MAX_REQUEST_BYTES {
            return Err(Error::RequestTooLarge {
                what: "images",
                bytes,
                limit: MAX_REQUEST_BYTES,
            });
        }
        self.images.insert(reference, image);
        Ok(())
    }

    /// The image under `reference`.
    pub fn get(&self, reference: &str) -> Option<&Image> {
        self.images.get(reference)
    }

    /// Every image with its reference, by reference.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Image)> {
        self.images.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Number of images.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// True without images: nothing is sent, and the request is as it was.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// The images' bytes together: what [`crate::Observer::on_assets`]
    /// reports and what [`MAX_REQUEST_BYTES`] bounds.
    pub fn total_bytes(&self) -> u64 {
        self.images
            .values()
            .map(|i| u64::try_from(i.len()).unwrap_or(u64::MAX))
            .sum()
    }

    /// Each reference to its image's [`Image::digest`]: what a request
    /// fingerprint holds for the images, so the fingerprint names the bytes
    /// and not their encoding ([`crate::eval::canonical::request_fingerprint_with`]).
    pub fn digests(&self) -> BTreeMap<String, String> {
        self.images
            .iter()
            .map(|(k, v)| (k.clone(), v.digest.clone()))
            .collect()
    }

    /// Refuse a reference that appears nowhere in the request, as the API
    /// does with a 400. `state_json` and `questions_json` are the two as
    /// serialised; a reference is matched as the JSON text of its string,
    /// so one with a quote or a backslash is found as the wire carries it.
    pub fn check_referenced(&self, state_json: &str, questions_json: &str) -> Result<()> {
        for reference in self.images.keys() {
            let quoted = serde_json::to_string(reference).unwrap_or_default();
            let text = quoted.trim_matches('"');
            if !state_json.contains(text) && !questions_json.contains(text) {
                return Err(Error::InvalidAsset {
                    reference: reference.clone(),
                    reason: "is referenced nowhere in the state or the questions; write the reference where the image belongs".to_owned(),
                });
            }
        }
        Ok(())
    }
}

/// The wire's `assets` map, each entry an [`Image`].
impl Serialize for Assets {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.images.serialize(serializer)
    }
}

/// `sha256:` and the lowercase hex of `bytes`' SHA-256.
fn digest_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(7 + 64);
    hex.push_str("sha256:");
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The PNG signature.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The format, width and height read from the file's own bytes, or why they
/// cannot be: not a PNG or a JPEG, a truncated header, an animated PNG.
fn sniff(bytes: &[u8]) -> std::result::Result<(ImageFormat, u32, u32), String> {
    if bytes.starts_with(&PNG_SIGNATURE) {
        let (width, height) = png_size(bytes)?;
        Ok((ImageFormat::Png, width, height))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        let (width, height) = jpeg_size(bytes)?;
        Ok((ImageFormat::Jpeg, width, height))
    } else {
        Err(
            "not a PNG or a JPEG (the format is read from the bytes, not from a file name)"
                .to_owned(),
        )
    }
}

fn be_u32(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn be_u16(bytes: &[u8], at: usize) -> Option<u16> {
    bytes
        .get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
}

/// The `IHDR` width and height; refused when the chunks before the first
/// `IDAT` include `acTL`, which marks an animated PNG.
fn png_size(bytes: &[u8]) -> std::result::Result<(u32, u32), String> {
    let damaged = || "a damaged or incomplete PNG: no IHDR chunk".to_owned();
    if bytes.get(12..16) != Some(b"IHDR") {
        return Err(damaged());
    }
    let width = be_u32(bytes, 16).ok_or_else(damaged)?;
    let height = be_u32(bytes, 20).ok_or_else(damaged)?;
    let mut at = 8;
    while let (Some(length), Some(kind)) = (be_u32(bytes, at), bytes.get(at + 4..at + 8)) {
        match kind {
            b"IDAT" | b"IEND" => break,
            b"acTL" => return Err("an animated PNG; the API takes still images only".to_owned()),
            _ => {}
        }
        at = at
            .checked_add(12)
            .and_then(|n| n.checked_add(usize::try_from(length).ok()?))
            .ok_or_else(damaged)?;
    }
    Ok((width, height))
}

/// The width and height of the first frame header (`SOF0` to `SOF15`,
/// but for the three markers that are not frames).
fn jpeg_size(bytes: &[u8]) -> std::result::Result<(u32, u32), String> {
    let damaged = || "a damaged or incomplete JPEG: no frame header".to_owned();
    let mut at = 2;
    loop {
        // Fill bytes before a marker are allowed.
        while bytes.get(at) == Some(&0xFF) {
            at += 1;
        }
        let marker = *bytes.get(at).ok_or_else(damaged)?;
        at += 1;
        match marker {
            // SOI, TEM, RSTn: no segment.
            0xD8 | 0x01 | 0xD0..=0xD7 => {}
            // EOI before a frame header.
            0xD9 => return Err(damaged()),
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                let height = be_u16(bytes, at + 3).ok_or_else(damaged)?;
                let width = be_u16(bytes, at + 5).ok_or_else(damaged)?;
                return Ok((u32::from(width), u32::from(height)));
            }
            _ => {
                let length = usize::from(be_u16(bytes, at).ok_or_else(damaged)?);
                at = at.checked_add(length).ok_or_else(damaged)?;
            }
        }
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with `=` padding and no line breaks, as the API requires.
pub(crate) fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().fold(0u32, |n, b| (n << 8) | u32::from(*b)) << (8 * (3 - chunk.len()));
        let sextets = [(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63, n & 63];
        for (i, s) in sextets.iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(BASE64[*s as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Strict base64: the standard alphabet, correct `=` padding, nothing else,
/// so what a document holds is exactly what the API accepts.
pub(crate) fn decode_base64(text: &str) -> std::result::Result<Vec<u8>, String> {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return Err("is empty".to_owned());
    }
    if !bytes.len().is_multiple_of(4) {
        return Err("is not a multiple of four characters; use standard base64 with `=` padding and no line breaks".to_owned());
    }
    let value = |b: u8| -> std::result::Result<u32, String> {
        BASE64
            .iter()
            .position(|c| *c == b)
            .map(|p| u32::try_from(p).unwrap_or(0))
            .ok_or_else(|| match b {
                b'\n' | b'\r' | b' ' | b'\t' => {
                    "holds whitespace; use one line with no breaks".to_owned()
                }
                b'-' | b'_' => "uses the URL-safe alphabet; use the standard one".to_owned(),
                _ => format!("holds a character outside the base64 alphabet (byte {b:#04x})"),
            })
    };
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let last = bytes.len() / 4 - 1;
    for (i, quad) in bytes.chunks(4).enumerate() {
        let padding = quad.iter().rev().take_while(|b| **b == b'=').count();
        if padding > 2 || (padding > 0 && i != last) || quad[..4 - padding].contains(&b'=') {
            return Err("has `=` padding where none belongs".to_owned());
        }
        let mut n = 0u32;
        for b in &quad[..4 - padding] {
            n = (n << 6) | value(*b)?;
        }
        n <<= 6 * padding;
        let triple = n.to_be_bytes();
        out.extend_from_slice(&triple[1..4 - padding]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    pub(crate) fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/images")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn base64_round_trips_and_is_strict() {
        for (bytes, text) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode_base64(bytes), text);
            if !text.is_empty() {
                assert_eq!(decode_base64(text).unwrap(), bytes);
            }
        }
        for (bad, needle) in [
            ("", "empty"),
            ("Zm9", "multiple of four"),
            ("Zm9v\nYmF", "whitespace"),
            ("Zm9v-mFy", "URL-safe"),
            ("Zm=v", "padding"),
            ("Zg==Zg==", "padding"),
            ("Z===", "padding"),
            ("Zm9v*mFy", "outside"),
        ] {
            let err = decode_base64(bad).unwrap_err();
            assert!(err.contains(needle), "{bad:?}: {err}");
        }
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(decode_base64(&encode_base64(&bytes)).unwrap(), bytes);
    }

    #[test]
    fn the_format_and_the_size_are_read_from_the_bytes() {
        let png = Image::from_bytes(fixture("square-300.png")).unwrap();
        assert_eq!(png.format(), ImageFormat::Png);
        assert_eq!((png.width(), png.height()), (300, 300));
        let jpeg = Image::from_bytes(fixture("square-300.jpg")).unwrap();
        assert_eq!(jpeg.format(), ImageFormat::Jpeg);
        assert_eq!((jpeg.width(), jpeg.height()), (300, 300));
        let progressive = Image::from_bytes(fixture("progressive-300.jpg")).unwrap();
        assert_eq!((progressive.width(), progressive.height()), (300, 300));
        assert!(png.digest().starts_with("sha256:"));
        assert_eq!(png.digest().len(), 7 + 64);
        assert_eq!(png.reference().len(), 4 + 32);
        assert!(png.reference().starts_with("img-"));
        assert_ne!(png.reference(), jpeg.reference());
        assert_eq!(
            Image::from_base64(&png.to_base64(), Some("png")).unwrap(),
            png
        );
        assert_eq!(
            Image::from_base64(&jpeg.to_base64(), Some("jpeg")).unwrap(),
            jpeg
        );
        assert_eq!(Image::from_base64(&jpeg.to_base64(), None).unwrap(), jpeg);
        let shown = format!("{png:?}");
        assert!(shown.contains("Png") && shown.contains("300"), "{shown}");
        assert!(!shown.contains("iVBOR"), "the bytes are never printed");
    }

    #[test]
    fn what_the_api_refuses_is_refused_before_the_wire() {
        for (name, needle) in [
            ("small-128.png", "128 pixels wide"),
            ("narrow-256x300.png", "76800 pixels"),
            ("wide-1600x300.png", "1600 pixels wide"),
            ("animated-300.png", "animated"),
            ("not-an-image.txt", "not a PNG or a JPEG"),
        ] {
            let err = Image::from_bytes(fixture(name)).unwrap_err();
            assert!(
                matches!(&err, Error::InvalidImage { reason } if reason.contains(needle)),
                "{name}: {err}"
            );
        }
        let truncated = &fixture("square-300.png")[..20];
        let err = Image::from_bytes(truncated).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidImage { reason } if reason.contains("incomplete")),
            "{err}"
        );
        let truncated = &fixture("square-300.jpg")[..40];
        let err = Image::from_bytes(truncated).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidImage { reason } if reason.contains("incomplete")),
            "{err}"
        );
        let err = Image::from_base64("not base64!", None).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidImage { reason } if reason.contains("data_base64")),
            "{err}"
        );
        let png = fixture("square-300.png");
        let err = Image::from_base64(&encode_base64(&png), Some("jpg")).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidImage { reason } if reason.contains("declared as jpg")),
            "{err}"
        );
        let err = Image::from_base64(&encode_base64(&png), Some("webp")).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidImage { reason } if reason.contains("webp")),
            "{err}"
        );
        let err = Image::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/images/small-128.png"),
        )
        .unwrap_err();
        assert!(
            matches!(&err, Error::InvalidImage { reason } if reason.contains("small-128.png")),
            "{err}"
        );
        let err = Image::read("/nonexistent/door.png").unwrap_err();
        assert!(matches!(err, Error::Io { .. }), "{err}");
    }

    #[test]
    fn assets_serialise_as_the_wire_map_and_name_images_by_digest() {
        let png = Image::from_bytes(fixture("square-300.png")).unwrap();
        let mut assets = Assets::new();
        assert!(assets.is_empty());
        let reference = assets.add(png.clone()).unwrap();
        assert_eq!(reference, png.reference());
        // The same image again is one entry.
        assert_eq!(assets.add(png.clone()).unwrap(), reference);
        assert_eq!(assets.len(), 1);
        assets
            .insert(
                "door",
                Image::from_bytes(fixture("square-300.jpg")).unwrap(),
            )
            .unwrap();
        let wire = serde_json::to_value(&assets).unwrap();
        assert_eq!(wire[&reference]["type"], "image");
        assert_eq!(wire[&reference]["format"], "png");
        assert_eq!(wire[&reference]["data_base64"], png.to_base64());
        assert_eq!(wire["door"]["format"], "jpg");
        let digests = assets.digests();
        assert_eq!(digests[&reference], png.digest());
        assert_eq!(assets.total_bytes(), 717 + 3357);
        assert_eq!(assets.get("door").unwrap().format(), ImageFormat::Jpeg);
        let keys: Vec<&str> = assets.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["door", reference.as_str()], "sorted by reference");
    }

    #[test]
    fn a_reference_follows_the_pages_rules() {
        let png = Image::from_bytes(fixture("square-300.png")).unwrap();
        let jpeg = Image::from_bytes(fixture("square-300.jpg")).unwrap();
        let mut assets = Assets::new();
        for (bad, needle) in [("", "empty"), ("  ", "empty"), (&"x".repeat(1025), "1024")] {
            let err = assets.insert(bad, png.clone()).unwrap_err();
            assert!(
                matches!(&err, Error::InvalidAsset { reason, .. } if reason.contains(needle)),
                "{bad:?}: {err}"
            );
        }
        assets.insert("door", png.clone()).unwrap();
        assets.insert("door", png.clone()).unwrap();
        let err = assets.insert("door", jpeg).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidAsset { reference, reason } if reference == "door" && reason.contains("another image")),
            "{err}"
        );
        let mut many = Assets::new();
        for i in 0..64 {
            many.insert(format!("{i:0>1024}"), png.clone()).unwrap();
        }
        let err = many.insert("one-more", png).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidAsset { reason, .. } if reason.contains("65536")),
            "{err}"
        );
    }

    #[test]
    fn an_unreferenced_asset_is_refused() {
        let png = Image::from_bytes(fixture("square-300.png")).unwrap();
        let mut assets = Assets::new();
        let reference = assets.add(png).unwrap();
        let state = serde_json::json!({ "photo": reference });
        let questions = serde_json::json!({});
        assets
            .check_referenced(&state.to_string(), &questions.to_string())
            .unwrap();
        let err = assets
            .check_referenced("{\"photo\":\"other\"}", "{}")
            .unwrap_err();
        assert!(
            matches!(&err, Error::InvalidAsset { reference: r, reason } if *r == reference && reason.contains("nowhere")),
            "{err}"
        );
        // Inside a longer string, in the questions, and with a quote.
        let mut quoted = Assets::new();
        quoted
            .insert(
                "say \"cheese\"",
                Image::from_bytes(fixture("square-300.jpg")).unwrap(),
            )
            .unwrap();
        let questions = serde_json::json!({ "q": { "instructions": "like this: say \"cheese\"" } });
        quoted
            .check_referenced("\"text\"", &questions.to_string())
            .unwrap();
    }
}
