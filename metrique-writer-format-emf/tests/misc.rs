// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use metrique_writer::{
    Entry, EntryIoStream, EntrySink, EntryWriter, FormatExt, sink::BackgroundQueue,
};
use metrique_writer_core::test_stream::TestSink;
use metrique_writer_format_emf::Emf;

struct TestEntry {
    count: u64,
}

impl Entry for TestEntry {
    fn write<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs_f64(12345.6789));
        writer.value("Time", &Duration::from_millis(42));
        writer.value("Operation", "Foo");
        writer.value("BasicIntCount", &self.count);
    }
}

#[test]
fn test_output_to_make_writer() {
    let output = Mutex::new(Vec::new());
    let mut stream =
        Emf::all_validations("MyApp".into(), vec![vec![]]).output_to_makewriter(|| {
            let mut output = output.lock().unwrap();
            output.push(TestSink::default());
            output.last_mut().unwrap().clone()
        });
    // create 2 entries to make sure both are recorded
    stream.next(&TestEntry { count: 1 }).unwrap();
    stream.next(&TestEntry { count: 2 }).unwrap();
    stream.flush().unwrap();

    let output = output.into_inner().unwrap();
    assert_eq!(output.len(), 2);
    assert_json_diff::assert_json_eq!(
        serde_json::from_str::<serde_json::Value>(&output[0].dump()).unwrap(),
        serde_json::json!({
            "_aws": {
                "CloudWatchMetrics": [{
                    "Namespace": "MyApp",
                    "Dimensions": [[]],
                    "Metrics": [
                        {"Name": "Time", "Unit": "Milliseconds"},
                        {"Name":"BasicIntCount"}
                    ]
                }],
                "Timestamp": 12345678
            },
            "Time": 42,
            "BasicIntCount": 1,
            "Operation":"Foo"
        })
    );
    assert_json_diff::assert_json_eq!(
        serde_json::from_str::<serde_json::Value>(&output[1].dump()).unwrap(),
        serde_json::json!({
            "_aws": {
                "CloudWatchMetrics": [{
                    "Namespace": "MyApp",
                    "Dimensions": [[]],
                    "Metrics": [
                        {"Name": "Time", "Unit": "Milliseconds"},
                        {"Name":"BasicIntCount"}
                    ]
                }],
                "Timestamp": 12345678
            },
            "Time": 42,
            "BasicIntCount": 2,
            "Operation":"Foo"
        })
    );
}

#[test]
fn test_background_queue_with_invalid_metric() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let output_ = output.clone();
    // this will cause a validation eror because BadDim is not provided
    let stream = Emf::all_validations("MyApp".into(), vec![vec!["BadDim".into()]])
        .output_to_makewriter(move || {
            let mut output = output_.lock().unwrap();
            output.push(TestSink::default());
            output.last_mut().unwrap().clone()
        });
    let (queue, jh) = BackgroundQueue::new(stream);
    queue.append(TestEntry { count: 1 });
    drop(jh);
    let m = output
        .lock()
        .unwrap()
        .iter()
        .map(|m| m.take_string())
        .collect::<Vec<_>>();
    // first entry is an empty entry for the invalid one
    assert_eq!(m[0], "");
    // second entry is a property for the bad entry
    let mut entry = serde_json::from_str::<serde_json::Value>(&m[1]).unwrap();
    entry["_aws"]["Timestamp"] = 0.into();
    assert_json_diff::assert_json_eq!(
        entry,
        serde_json::json!({
            "_aws": {
                "CloudWatchMetrics": [{
                    "Namespace": "MyApp",
                    "Dimensions": [["BadDim"]],
                    "Metrics": []
                }],
                "Timestamp": 0
            },
            "MetriqueValidationError":
                "metric entry could not be formatted correctly, call tracing_subscriber::fmt::init to see more detailed information"
        })
    );
}

struct VecEntry {
    plugins: Vec<String>,
}

impl Entry for VecEntry {
    fn write<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs(1));
        writer.value("Plugins", &self.plugins);
    }
}

#[test]
fn test_vec_emits_json_array_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream
        .next(&VecEntry {
            plugins: vec!["auth".into(), "logging".into(), "cache".into()],
        })
        .unwrap();
    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();
    assert_json_diff::assert_json_eq!(
        output["Plugins"],
        serde_json::json!(["auth", "logging", "cache"])
    );
}

#[test]
fn test_empty_vec_emits_empty_array_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream.next(&VecEntry { plugins: vec![] }).unwrap();
    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();
    assert_json_diff::assert_json_eq!(output["Plugins"], serde_json::json!([]));
}

struct VecOptionEntry {
    tags: Vec<Option<String>>,
}

impl Entry for VecOptionEntry {
    fn write<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs(1));
        writer.value("Tags", &self.tags);
    }
}

#[test]
fn test_vec_with_none_elements_skips_them_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream
        .next(&VecOptionEntry {
            tags: vec![Some("a".into()), None, Some("c".into())],
        })
        .unwrap();
    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();
    assert_json_diff::assert_json_eq!(output["Tags"], serde_json::json!(["a", "c"]));
}

#[test]
fn test_single_element_vec_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream
        .next(&VecEntry {
            plugins: vec!["only".into()],
        })
        .unwrap();
    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();
    assert_json_diff::assert_json_eq!(output["Plugins"], serde_json::json!(["only"]));
}

#[test]
fn test_vec_emits_json_array_through_boxed_entry() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    let boxed = VecEntry {
        plugins: vec!["a".into(), "b".into()],
    }
    .boxed();
    stream.next(&boxed).unwrap();
    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();
    assert_json_diff::assert_json_eq!(output["Plugins"], serde_json::json!(["a", "b"]));
}

struct VecU64Entry {
    counts: Vec<u64>,
}

impl Entry for VecU64Entry {
    fn write<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs(1));
        writer.value("Counts", &self.counts);
    }
}

#[test]
fn test_vec_u64_emits_json_array_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream
        .next(&VecU64Entry {
            counts: vec![10, 20, 30],
        })
        .unwrap();
    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();
    assert_json_diff::assert_json_eq!(output["Counts"], serde_json::json!([10, 20, 30]));
}

#[test]
fn test_single_u64_vec_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream.next(&VecU64Entry { counts: vec![42] }).unwrap();
    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();
    assert_json_diff::assert_json_eq!(output["Counts"], serde_json::json!([42]));
}

#[test]
fn test_empty_u64_vec_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream.next(&VecU64Entry { counts: vec![] }).unwrap();
    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();
    assert_json_diff::assert_json_eq!(output["Counts"], serde_json::json!([]));
}

/// A custom Value that emits multiple observations, exercising the nested
/// sub-array path in EmfArrayElementWriter::metric().
struct MultiObsValue(Vec<u64>);

impl metrique_writer_core::Value for MultiObsValue {
    fn write(&self, writer: impl metrique_writer_core::ValueWriter) {
        writer.metric(
            self.0
                .iter()
                .map(|&v| metrique_writer_core::Observation::Unsigned(v)),
            metrique_writer_core::Unit::None,
            [],
            metrique_writer_core::MetricFlags::empty(),
        );
    }
}

struct VecMultiObsEntry {
    data: Vec<MultiObsValue>,
}

impl Entry for VecMultiObsEntry {
    fn write<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs(1));
        writer.value("Data", &self.data);
    }
}

#[test]
fn test_vec_multi_observation_nests_sub_arrays_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream
        .next(&VecMultiObsEntry {
            data: vec![MultiObsValue(vec![1, 2, 3]), MultiObsValue(vec![4, 5])],
        })
        .unwrap();
    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();
    assert_json_diff::assert_json_eq!(output["Data"], serde_json::json!([[1, 2, 3], [4, 5]]));
}

#[test]
fn test_vec_single_observation_stays_scalar_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream
        .next(&VecMultiObsEntry {
            data: vec![MultiObsValue(vec![10]), MultiObsValue(vec![20])],
        })
        .unwrap();
    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();
    assert_json_diff::assert_json_eq!(output["Data"], serde_json::json!([10, 20]));
}

// ============================================================
// Object value tests
// ============================================================

use metrique_writer::value::{ObjectValue, Value};

/// A simple phase struct simulating what the macro would generate.
struct Phase {
    phase_type: &'static str,
    duration: u64,
}

impl ObjectValue for Phase {
    fn write_object<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.value("Type", &self.phase_type);
        writer.value("Duration", &self.duration);
    }
}

/// A wrapper that calls `writer.object()` — simulates what a `Value` impl
/// generated by `AsObject` would do.
struct AsObjectWrapper<'a>(&'a Phase);

impl Value for AsObjectWrapper<'_> {
    fn write(&self, writer: impl metrique_writer::ValueWriter) {
        writer.object(self.0);
    }
}

/// Entry that contains a single object field.
struct ObjectEntry {
    phase: Phase,
}

impl Entry for ObjectEntry {
    fn write<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs(1));
        writer.value("RequestId", "abc-123");
        writer.value("RootPhase", &AsObjectWrapper(&self.phase));
    }
}

#[test]
fn test_object_renders_as_native_json_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream
        .next(&ObjectEntry {
            phase: Phase {
                phase_type: "parse_expr",
                duration: 11,
            },
        })
        .unwrap();

    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();

    // The object should appear as a native JSON object in the body
    assert_json_diff::assert_json_eq!(
        output["RootPhase"],
        serde_json::json!({"Type": "parse_expr", "Duration": 11})
    );

    // The object fields must NOT appear in _aws CloudWatchMetrics
    let metrics = &output["_aws"]["CloudWatchMetrics"][0]["Metrics"];
    let metrics_arr = metrics.as_array().unwrap();
    // No metric named "Type", "Duration", or "RootPhase" should exist
    for m in metrics_arr {
        let name = m["Name"].as_str().unwrap();
        assert_ne!(name, "Type");
        assert_ne!(name, "Duration");
        assert_ne!(name, "RootPhase");
    }
}

/// A recursive tree: Phase with children.
struct TreePhase {
    phase_type: &'static str,
    duration: u64,
    children: Vec<TreePhase>,
}

impl ObjectValue for TreePhase {
    fn write_object<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.value("Type", &self.phase_type);
        writer.value("Duration", &self.duration);
        if !self.children.is_empty() {
            let wrapped: Vec<AsObjectWrapper2<'_>> =
                self.children.iter().map(|c| AsObjectWrapper2(c)).collect();
            writer.value("Phases", &wrapped);
        }
    }
}

/// Wrapper that treats TreePhase as a Value (calls object).
struct AsObjectWrapper2<'a>(&'a TreePhase);

impl Value for AsObjectWrapper2<'_> {
    fn write(&self, writer: impl metrique_writer::ValueWriter) {
        writer.object(self.0);
    }
}

/// Entry that has a Vec of tree phases.
struct TreeEntry {
    phases: Vec<TreePhase>,
}

impl Entry for TreeEntry {
    fn write<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs(1));
        writer.value("RequestId", "xyz-789");
        let wrapped: Vec<AsObjectWrapper2<'_>> =
            self.phases.iter().map(|p| AsObjectWrapper2(p)).collect();
        writer.value("Phases", &wrapped);
    }
}

#[test]
fn test_recursive_object_tree_in_emf() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream
        .next(&TreeEntry {
            phases: vec![
                TreePhase {
                    phase_type: "parse_expr",
                    duration: 11,
                    children: vec![],
                },
                TreePhase {
                    phase_type: "eval",
                    duration: 22,
                    children: vec![TreePhase {
                        phase_type: "multiply",
                        duration: 33,
                        children: vec![
                            TreePhase {
                                phase_type: "add",
                                duration: 44,
                                children: vec![],
                            },
                            TreePhase {
                                phase_type: "sub",
                                duration: 55,
                                children: vec![],
                            },
                        ],
                    }],
                },
            ],
        })
        .unwrap();

    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();

    // Verify the recursive structure
    assert_json_diff::assert_json_eq!(
        output["Phases"],
        serde_json::json!([
            {"Type": "parse_expr", "Duration": 11},
            {
                "Type": "eval",
                "Duration": 22,
                "Phases": [
                    {
                        "Type": "multiply",
                        "Duration": 33,
                        "Phases": [
                            {"Type": "add", "Duration": 44},
                            {"Type": "sub", "Duration": 55}
                        ]
                    }
                ]
            }
        ])
    );

    // Verify no _aws metric directives reference any object fields
    let metrics = &output["_aws"]["CloudWatchMetrics"][0]["Metrics"];
    assert_json_diff::assert_json_eq!(metrics, serde_json::json!([]));
}

/// Test that a metric-typed field inside an object renders as a bare number
/// (no unit, no dimensions).
struct PhaseWithMetric {
    name: &'static str,
    latency_ms: u64,
}

impl ObjectValue for PhaseWithMetric {
    fn write_object<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.value("Name", &self.name);
        // This writes a metric with unit — but inside an object, it should
        // render as a bare number.
        writer.value(
            "Latency",
            &metrique_writer::unit::AsMilliseconds::from(self.latency_ms),
        );
    }
}

struct MetricInObjectEntry {
    phase: PhaseWithMetric,
}

impl Entry for MetricInObjectEntry {
    fn write<'a>(&'a self, writer: &mut impl EntryWriter<'a>) {
        writer.timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs(1));
        writer.value("Phase", &AsObjectWrapperMetric(&self.phase));
    }
}

struct AsObjectWrapperMetric<'a>(&'a PhaseWithMetric);

impl Value for AsObjectWrapperMetric<'_> {
    fn write(&self, writer: impl metrique_writer::ValueWriter) {
        writer.object(self.0);
    }
}

#[test]
fn test_metric_inside_object_renders_as_bare_number() {
    let sink = TestSink::default();
    let mut stream = Emf::all_validations("App".into(), vec![vec![]]).output_to(sink.clone());
    stream
        .next(&MetricInObjectEntry {
            phase: PhaseWithMetric {
                name: "db_query",
                latency_ms: 42,
            },
        })
        .unwrap();

    let output: serde_json::Value = serde_json::from_str(&sink.dump()).unwrap();

    // The metric should appear as a bare number inside the object
    assert_json_diff::assert_json_eq!(
        output["Phase"],
        serde_json::json!({"Name": "db_query", "Latency": 42})
    );

    // No _aws metric for "Latency" or "Phase"
    let metrics = &output["_aws"]["CloudWatchMetrics"][0]["Metrics"];
    let metrics_arr = metrics.as_array().unwrap();
    for m in metrics_arr {
        let name = m["Name"].as_str().unwrap();
        assert_ne!(name, "Latency");
        assert_ne!(name, "Phase");
    }
}
