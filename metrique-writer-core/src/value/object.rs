// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! The [`ObjectValue`] trait for types that can be rendered as nested objects.

use crate::EntryWriter;

/// A type whose fields can be emitted as members of a nested JSON object.
///
/// Each call to `writer.value(name, v)` inside `write_object` becomes a
/// `"name": <v>` member of the object. Calls to `timestamp` and `config`
/// on the writer are ignored (an object has no timestamp and no entry-level
/// configuration).
///
/// `ObjectValue` is obtained two ways:
/// - The `#[metrics]` derive generates it for every entry-producing struct.
/// - A user hand-implements it for a type they do not own (through a newtype).
pub trait ObjectValue {
    /// Emit this object's members into the given writer.
    fn write_object<'a>(&'a self, writer: &mut impl EntryWriter<'a>);
}
