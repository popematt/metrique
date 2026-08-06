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

/// A [`ValueFormatter`](super::ValueFormatter) that renders its value as a nested object
/// by delegating to [`ObjectValue::write_object`].
///
/// Use with `#[metrics(format = AsObject)]` on fields whose type implements `ObjectValue`
/// (which the `#[metrics]` derive generates automatically for entry-producing structs).
///
/// `AsObject` has no blanket library impl. Instead, the `#[metrics]` derive generates a
/// concrete `Lifted` `impl ValueFormatter<FooEntry> for AsObject` per entry type. For
/// hand-implemented types, the user writes the same concrete impl. This design ensures
/// unambiguous `L`-inference and enables auto-lifting through `Option`/`Box`/`Arc`/`Cow`.
pub struct AsObject;

/// A [`ValueFormatter`](super::ValueFormatter) that renders each element of an iterable
/// using an inner formatter, emitting them as a list via [`ValueWriter::values`](super::ValueWriter::values).
///
/// Use with `#[metrics(format(Each<InnerFormatter>))]` on iterable fields (e.g., `Vec<T>`).
///
/// `Each<F>` uses the default `Lifted` liftability, so `Option<Vec<T>>` auto-lifts.
/// The inner formatter `F` must implement `ValueFormatter<T>` (also `Lifted`).
pub struct Each<F>(std::marker::PhantomData<F>);

impl<T, F> super::ValueFormatter<Vec<T>> for Each<F>
where
    F: super::ValueFormatter<T>,
{
    const SHAPE: crate::descriptor::FieldShape<'static> = crate::descriptor::FieldShape::List(
        crate::descriptor::ShapeRef::new(&<F as super::ValueFormatter<T>>::SHAPE),
    );

    fn format_value(writer: impl super::ValueWriter, value: &Vec<T>) {
        let wrapped: Vec<super::FormattedValue<'_, T, F>> =
            value.iter().map(super::FormattedValue::new).collect();
        writer.values(wrapped.iter());
    }
}

impl<T, F, const N: usize> super::ValueFormatter<[T; N]> for Each<F>
where
    F: super::ValueFormatter<T>,
{
    const SHAPE: crate::descriptor::FieldShape<'static> = crate::descriptor::FieldShape::List(
        crate::descriptor::ShapeRef::new(&<F as super::ValueFormatter<T>>::SHAPE),
    );

    fn format_value(writer: impl super::ValueWriter, value: &[T; N]) {
        let wrapped: Vec<super::FormattedValue<'_, T, F>> =
            value.iter().map(super::FormattedValue::new).collect();
        writer.values(wrapped.iter());
    }
}
