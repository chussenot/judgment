//! The one check a caller's state gets before the client serialises it:
//! every number in it is finite.
//!
//! TypeSafe refuses `NaN` and the infinities with a 400
//! (<https://docs.typesafe.ai/api/errors.md>, "Request rules"), but
//! `serde_json` never lets one reach the wire: it writes a non-finite float
//! as `null`, with no error. The request would go out with the value gone and
//! the model would be asked about a state that lost it, which is the silent
//! loss the crate exists to prevent. So the client walks the state once with
//! [`Finite`], a serializer that writes nothing and refuses a non-finite
//! float, and the caller gets [`crate::Error::InvalidState`] before anything
//! is built or sent.
//!
//! Only the client needs it: every other backend takes a `serde_json::Value`,
//! which cannot hold a non-finite number.

use serde::Serialize;
use serde::ser::{self, Error as _};

/// Walk `state` and refuse it if it holds a non-finite `f32` or `f64`.
pub(crate) fn check<T: Serialize + ?Sized>(state: &T) -> Result<(), serde_json::Error> {
    state.serialize(Finite)
}

// ponytail: the error does not say where in the state the number sits; add a
// path when a caller asks for one.
fn refuse(value: f64) -> serde_json::Error {
    let what = if value.is_nan() {
        "NaN"
    } else if value > 0.0 {
        "an infinity"
    } else {
        "a negative infinity"
    };
    serde_json::Error::custom(format_args!(
        "the state holds {what}, a number JSON cannot carry: it would be sent as null \
         and TypeSafe refuses it; send it as a string or leave it out"
    ))
}

/// A serializer that writes nothing and refuses a non-finite float.
#[derive(Clone, Copy)]
struct Finite;

type Done = Result<(), serde_json::Error>;

impl ser::Serializer for Finite {
    type Ok = ();
    type Error = serde_json::Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_f32(self, v: f32) -> Done {
        self.serialize_f64(f64::from(v))
    }
    fn serialize_f64(self, v: f64) -> Done {
        if v.is_finite() {
            Ok(())
        } else {
            Err(refuse(v))
        }
    }

    fn serialize_bool(self, _: bool) -> Done {
        Ok(())
    }
    fn serialize_i8(self, _: i8) -> Done {
        Ok(())
    }
    fn serialize_i16(self, _: i16) -> Done {
        Ok(())
    }
    fn serialize_i32(self, _: i32) -> Done {
        Ok(())
    }
    fn serialize_i64(self, _: i64) -> Done {
        Ok(())
    }
    fn serialize_i128(self, _: i128) -> Done {
        Ok(())
    }
    fn serialize_u8(self, _: u8) -> Done {
        Ok(())
    }
    fn serialize_u16(self, _: u16) -> Done {
        Ok(())
    }
    fn serialize_u32(self, _: u32) -> Done {
        Ok(())
    }
    fn serialize_u64(self, _: u64) -> Done {
        Ok(())
    }
    fn serialize_u128(self, _: u128) -> Done {
        Ok(())
    }
    fn serialize_char(self, _: char) -> Done {
        Ok(())
    }
    fn serialize_str(self, _: &str) -> Done {
        Ok(())
    }
    fn serialize_bytes(self, _: &[u8]) -> Done {
        Ok(())
    }
    fn serialize_none(self) -> Done {
        Ok(())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Done {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Done {
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Done {
        Ok(())
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Done {
        Ok(())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _: &'static str, value: &T) -> Done {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        value: &T,
    ) -> Done {
        value.serialize(self)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self, serde_json::Error> {
        Ok(self)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self, serde_json::Error> {
        Ok(self)
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self, serde_json::Error> {
        Ok(self)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, serde_json::Error> {
        Ok(self)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self, serde_json::Error> {
        Ok(self)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, serde_json::Error> {
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, serde_json::Error> {
        Ok(self)
    }
}

impl ser::SerializeSeq for Finite {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Done {
        value.serialize(*self)
    }
    fn end(self) -> Done {
        Ok(())
    }
}

impl ser::SerializeTuple for Finite {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Done {
        value.serialize(*self)
    }
    fn end(self) -> Done {
        Ok(())
    }
}

impl ser::SerializeTupleStruct for Finite {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Done {
        value.serialize(*self)
    }
    fn end(self) -> Done {
        Ok(())
    }
}

impl ser::SerializeTupleVariant for Finite {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Done {
        value.serialize(*self)
    }
    fn end(self) -> Done {
        Ok(())
    }
}

impl ser::SerializeMap for Finite {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Done {
        key.serialize(*self)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Done {
        value.serialize(*self)
    }
    fn end(self) -> Done {
        Ok(())
    }
}

impl ser::SerializeStruct for Finite {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, value: &T) -> Done {
        value.serialize(*self)
    }
    fn end(self) -> Done {
        Ok(())
    }
}

impl ser::SerializeStructVariant for Finite {
    type Ok = ();
    type Error = serde_json::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, value: &T) -> Done {
        value.serialize(*self)
    }
    fn end(self) -> Done {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::collections::{BTreeMap, HashMap};

    use serde::Serialize;
    use serde_json::json;

    use super::check;

    #[derive(Serialize)]
    enum Reading {
        Point(f64),
        Pair(f32, f32),
        Named { value: f64 },
    }

    #[derive(Serialize)]
    struct State {
        label: String,
        readings: Vec<Option<Reading>>,
        by_name: BTreeMap<String, f64>,
    }

    fn state(value: f64) -> State {
        State {
            label: "probe".to_owned(),
            readings: vec![None, Some(Reading::Named { value })],
            by_name: BTreeMap::new(),
        }
    }

    #[test]
    fn a_non_finite_number_is_refused_wherever_it_sits() {
        for (bad, word) in [
            (f64::NAN, "NaN"),
            (f64::INFINITY, "an infinity"),
            (f64::NEG_INFINITY, "a negative infinity"),
        ] {
            let err = check(&state(bad)).unwrap_err().to_string();
            assert!(err.contains(&format!("holds {word},")), "{err}");
            assert!(err.contains("null"), "{err}");

            // In a sequence, an Option, a tuple variant, a map value, a
            // newtype variant, an f32, and a HashMap key.
            assert!(check(&vec![1.0, bad]).is_err());
            assert!(check(&Some(bad)).is_err());
            assert!(check(&Reading::Point(bad)).is_err());
            #[allow(clippy::cast_possible_truncation)]
            let narrow = bad as f32;
            assert!(check(&Reading::Pair(0.5, narrow)).is_err());
            let mut s = state(1.0);
            s.by_name.insert("ratio".to_owned(), bad);
            assert!(check(&s).is_err());
            assert!(check(&HashMap::from([(1u8, (2.0f64, bad))])).is_err());
        }
    }

    #[test]
    fn finite_numbers_and_everything_else_pass() {
        assert!(check(&state(0.25)).is_ok());
        for fine in [0.0, -0.0, f64::MAX, f64::MIN, f64::MIN_POSITIVE, 1e-300] {
            assert!(check(&state(fine)).is_ok(), "{fine}");
        }
        // A Value cannot hold a non-finite number at all.
        assert!(check(&json!({ "a": [1, 2.5, null, "NaN"], "b": { "c": true } })).is_ok());
        assert!(check(&(1u128, -1i128, 'x', b"bytes".as_slice(), ())).is_ok());
    }
}
