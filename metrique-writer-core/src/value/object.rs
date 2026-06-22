// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Write;
use std::sync::Arc;

use super::{MetricFlags, Observation, Value, ValueWriter};
use crate::{Unit, ValidationError};

/// Visitor for writing named fields of a nested object.
pub trait ObjectWriter {
    /// Write a named field.
    ///
    /// The value is rendered as a property, never as a declared metric.
    fn field(&mut self, name: &str, value: &(impl Value + ?Sized));
}

/// A value that can be written as a nested object with named fields.
pub trait ObjectValue {
    /// Visit each field of this object.
    fn write_object(&self, writer: &mut impl ObjectWriter);
}

impl<T: ObjectValue + ?Sized> ObjectValue for &T {
    fn write_object(&self, writer: &mut impl ObjectWriter) {
        (**self).write_object(writer)
    }
}

impl<T: ObjectValue + ?Sized> ObjectValue for Box<T> {
    fn write_object(&self, writer: &mut impl ObjectWriter) {
        (**self).write_object(writer)
    }
}

impl<T: ObjectValue + ?Sized> ObjectValue for Arc<T> {
    fn write_object(&self, writer: &mut impl ObjectWriter) {
        (**self).write_object(writer)
    }
}

/// Default [`ObjectWriter`] used by the [`ValueWriter::object`] fallback.
pub(crate) struct DefaultObjectWriter<'a> {
    buf: &'a mut String,
    first: bool,
}

impl<'a> DefaultObjectWriter<'a> {
    pub(crate) fn new(buf: &'a mut String) -> Self {
        Self { buf, first: true }
    }
}

impl ObjectWriter for DefaultObjectWriter<'_> {
    fn field(&mut self, name: &str, value: &(impl Value + ?Sized)) {
        let before = self.buf.len();
        if !self.first {
            self.buf.push(',');
        }
        self.buf.push('"');
        json_escape_into(self.buf, name);
        self.buf.push_str("\":");
        let after_key = self.buf.len();
        value.write(ObjectValueCapture(self.buf));
        if self.buf.len() > after_key {
            self.first = false;
        } else {
            self.buf.truncate(before);
        }
    }
}

/// Captures a nested object field as a JSON fragment.
pub(crate) struct ObjectValueCapture<'a>(pub(crate) &'a mut String);

impl ValueWriter for ObjectValueCapture<'_> {
    fn string(self, value: &str) {
        self.0.push('"');
        json_escape_into(self.0, value);
        self.0.push('"');
    }

    fn metric<'a>(
        self,
        distribution: impl IntoIterator<Item = Observation>,
        _unit: Unit,
        _dimensions: impl IntoIterator<Item = (&'a str, &'a str)>,
        _flags: MetricFlags<'_>,
    ) {
        let mut iter = distribution.into_iter();
        let Some(first) = iter.next() else { return };
        match iter.next() {
            None => write_observation_json(self.0, first),
            Some(second) => {
                self.0.push('[');
                write_observation_json(self.0, first);
                self.0.push(',');
                write_observation_json(self.0, second);
                for obs in iter {
                    self.0.push(',');
                    write_observation_json(self.0, obs);
                }
                self.0.push(']');
            }
        }
    }

    fn values<'a, V: Value + 'a>(self, values: impl IntoIterator<Item = &'a V>) {
        let buf = self.0;
        buf.push('[');
        let mut wrote_any = false;
        for value in values {
            let before = buf.len();
            if wrote_any {
                buf.push(',');
            }
            let after_sep = buf.len();
            write_nested_json_value(buf, value);
            if buf.len() > after_sep {
                wrote_any = true;
            } else {
                buf.truncate(before);
            }
        }
        buf.push(']');
    }

    fn error(self, _error: ValidationError) {}

    fn object(self, value: &(impl ObjectValue + ?Sized)) {
        self.0.push('{');
        value.write_object(&mut DefaultObjectWriter::new(self.0));
        self.0.push('}');
    }
}

fn write_nested_json_value(buf: &mut String, value: &(impl Value + ?Sized)) {
    value.write(ObjectValueCapture(buf));
}

pub(crate) fn json_escape_into(buf: &mut String, value: &str) {
    let bytes = value.as_bytes();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        let escape = match b {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\n' => "\\n",
            b'\r' => "\\r",
            b'\t' => "\\t",
            0x00..=0x1f => {
                buf.push_str(&value[start..i]);
                start = i + 1;
                let _ = write!(buf, "\\u{b:04x}");
                continue;
            }
            _ => continue,
        };
        buf.push_str(&value[start..i]);
        buf.push_str(escape);
        start = i + 1;
    }
    buf.push_str(&value[start..]);
}

fn write_observation_json(buf: &mut String, obs: Observation) {
    match obs {
        Observation::Unsigned(v) => {
            let _ = write!(buf, "{v}");
        }
        Observation::Floating(v) => write_float_json(buf, v),
        Observation::Repeated { total, occurrences } => {
            buf.push_str("{\"total\":");
            write_float_json(buf, total);
            buf.push_str(",\"count\":");
            let _ = write!(buf, "{occurrences}");
            buf.push('}');
        }
    }
}

fn write_float_json(buf: &mut String, value: f64) {
    let value = value.clamp(-f64::MAX, f64::MAX);
    if value.is_nan() {
        buf.push_str("null");
    } else {
        let _ = write!(buf, "{value}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CaptureWriter<'a>(&'a mut String);

    impl ValueWriter for CaptureWriter<'_> {
        fn string(self, value: &str) {
            self.0.push_str(value);
        }

        fn metric<'a>(
            self,
            _distribution: impl IntoIterator<Item = Observation>,
            _unit: Unit,
            _dimensions: impl IntoIterator<Item = (&'a str, &'a str)>,
            _flags: MetricFlags<'_>,
        ) {
            unreachable!("object fallback should serialize as a string")
        }

        fn error(self, _error: ValidationError) {
            unreachable!("test value should not error")
        }
    }

    struct NestedObject;

    impl Value for NestedObject {
        fn write(&self, writer: impl ValueWriter) {
            writer.object(self)
        }
    }

    impl ObjectValue for NestedObject {
        fn write_object(&self, writer: &mut impl ObjectWriter) {
            writer.field("count", &2u64);
            writer.field("name", &"duck");
        }
    }

    struct ParentObject;

    impl Value for ParentObject {
        fn write(&self, writer: impl ValueWriter) {
            writer.object(self)
        }
    }

    impl ObjectValue for ParentObject {
        fn write_object(&self, writer: &mut impl ObjectWriter) {
            writer.field("message", &"hello\nworld");
            writer.field("value", &42u64);
            writer.field("nested", &NestedObject);
            writer.field("items", &vec![1u64, 2, 3]);
            writer.field("maybe", &Option::<u64>::None);
        }
    }

    #[test]
    fn default_object_writer_omits_none_and_emits_nested_json() {
        let mut buf = String::new();
        ParentObject.write(ObjectValueCapture(&mut buf));
        let json: serde_json::Value = serde_json::from_str(&buf).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "message": "hello\nworld",
                "value": 42,
                "nested": {
                    "count": 2,
                    "name": "duck",
                },
                "items": [1, 2, 3],
            })
        );
    }

    #[test]
    fn default_object_fallback_serializes_as_json_string() {
        let mut out = String::new();
        CaptureWriter(&mut out).object(&ParentObject);
        let object: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(object["nested"]["count"], 2);
        assert!(object.get("maybe").is_none());
    }
}
