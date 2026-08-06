// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for object value support: `#[metrics(format = AsObject)]`
//! and `#[metrics(format = Each<AsObject>)]`.

use metrique::emf::Emf;
use metrique::unit_of_work::metrics;
use metrique::writer::value::{AsObject, Each};
use metrique::writer::{Entry, format::Format};
use metrique::{CloseValue, RootEntry};
use metrique_writer_core::descriptor::FieldShape;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A nested phase that the macro should make `ObjectValue` for.
#[metrics(rename_all = "PascalCase")]
struct Phase {
    kind: &'static str,
    duration_ms: u64,
}

/// A recursive tree phase with children rendered as a list of objects.
#[metrics(rename_all = "PascalCase")]
struct TreePhase {
    name: &'static str,
    duration_ms: u64,
    #[metrics(format = Each<AsObject>)]
    children: Vec<TreePhase>,
}

/// Top-level request metrics with a single object field and a list of objects.
#[metrics(rename_all = "PascalCase")]
struct RequestMetrics {
    request_id: &'static str,
    #[metrics(timestamp)]
    timestamp: SystemTime,
    #[metrics(format = AsObject)]
    root_phase: Phase,
    #[metrics(format = Each<AsObject>)]
    phases: Vec<Phase>,
}

/// Test that Option<ObjectValue> auto-lifts through the concrete Lifted impl.
#[metrics(rename_all = "PascalCase")]
struct OptionalObjectMetrics {
    #[metrics(timestamp)]
    timestamp: SystemTime,
    #[metrics(format = AsObject)]
    maybe_phase: Option<Phase>,
}

#[test]
fn object_value_compiles_and_formats_emf() {
    let mut emf = Emf::no_validations("App".into(), vec![vec![]]);
    let mut output = vec![];

    let m = RequestMetrics {
        request_id: "abc-123",
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        root_phase: Phase {
            kind: "parse",
            duration_ms: 42,
        },
        phases: vec![
            Phase {
                kind: "lex",
                duration_ms: 10,
            },
            Phase {
                kind: "codegen",
                duration_ms: 32,
            },
        ],
    };

    let closed = m.close();
    let entry = RootEntry::new(closed);
    emf.format(&entry, &mut output).unwrap();

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output).unwrap()).unwrap();

    // RootPhase should be a nested JSON object
    assert_eq!(json["RootPhase"]["Kind"], "parse");
    assert_eq!(json["RootPhase"]["DurationMs"], 42);

    // Phases should be a JSON array of objects
    let phases = json["Phases"].as_array().unwrap();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0]["Kind"], "lex");
    assert_eq!(phases[0]["DurationMs"], 10);
    assert_eq!(phases[1]["Kind"], "codegen");
    assert_eq!(phases[1]["DurationMs"], 32);

    // RequestId should be a string property
    assert_eq!(json["RequestId"], "abc-123");
}

#[test]
fn recursive_object_tree() {
    let mut emf = Emf::no_validations("App".into(), vec![vec![]]);
    let mut output = vec![];

    let m = TreePhase {
        name: "root",
        duration_ms: 100,
        children: vec![
            TreePhase {
                name: "child1",
                duration_ms: 40,
                children: vec![],
            },
            TreePhase {
                name: "child2",
                duration_ms: 60,
                children: vec![TreePhase {
                    name: "grandchild",
                    duration_ms: 30,
                    children: vec![],
                }],
            },
        ],
    };

    let closed = m.close();
    let entry = RootEntry::new(closed);
    emf.format(&entry, &mut output).unwrap();

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output).unwrap()).unwrap();

    assert_eq!(json["Name"], "root");
    assert_eq!(json["DurationMs"], 100);

    let children = json["Children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0]["Name"], "child1");
    assert_eq!(children[1]["Name"], "child2");

    let grandchildren = children[1]["Children"].as_array().unwrap();
    assert_eq!(grandchildren.len(), 1);
    assert_eq!(grandchildren[0]["Name"], "grandchild");
}

#[test]
fn option_object_auto_lifts() {
    let mut emf = Emf::no_validations("App".into(), vec![vec![]]);

    // Some case: object renders
    let m = OptionalObjectMetrics {
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        maybe_phase: Some(Phase {
            kind: "parse",
            duration_ms: 42,
        }),
    };
    let mut output = vec![];
    let closed = m.close();
    let entry = RootEntry::new(closed);
    emf.format(&entry, &mut output).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output).unwrap()).unwrap();
    assert_eq!(json["MaybePhase"]["Kind"], "parse");
    assert_eq!(json["MaybePhase"]["DurationMs"], 42);

    // None case: field omitted
    let m = OptionalObjectMetrics {
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        maybe_phase: None,
    };
    let mut output = vec![];
    let closed = m.close();
    let entry = RootEntry::new(closed);
    emf.format(&entry, &mut output).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output).unwrap()).unwrap();
    assert!(json.get("MaybePhase").is_none());
}

#[test]
fn object_shape_in_descriptors() {
    let m = RequestMetrics {
        request_id: "x",
        timestamp: UNIX_EPOCH,
        root_phase: Phase {
            kind: "a",
            duration_ms: 0,
        },
        phases: vec![],
    };

    let closed = m.close();
    let entry = RootEntry::new(closed);
    let descs = entry.descriptors().unwrap();
    let fields: Vec<_> = descs.iter().flat_map(|d| d.fields()).collect();

    // Fields: RequestId, RootPhase, Phases
    assert_eq!(fields.len(), 3);

    // RequestId should have a string shape
    assert_eq!(fields[0].base_name(), "RequestId");

    // RootPhase should have Object shape
    assert_eq!(fields[1].base_name(), "RootPhase");
    assert_eq!(fields[1].shape(), FieldShape::Object);

    // Phases should have List(Object) shape
    assert_eq!(fields[2].base_name(), "Phases");
    assert!(
        matches!(fields[2].shape(), FieldShape::List(inner) if *inner.get() == FieldShape::Object)
    );
}
