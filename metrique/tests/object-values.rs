use metrique::emf::Emf;
use metrique::writer::{ObjectValue, format::Format, test_util};
use metrique::{CloseValue, RootEntry, unit_of_work::metrics};
use std::sync::Arc as StdArc;

#[metrics(value(object))]
struct RequestDetail {
    shard: &'static str,
    attempt: u64,
}

#[metrics(value(object))]
struct RequestContext {
    request_id: &'static str,
    retries: u64,
    detail: RequestDetail,
    maybe_detail: Option<RequestDetail>,
}

#[metrics(value(object))]
struct SingleFieldObject {
    count: u64,
}

#[metrics]
struct Metrics {
    context: RequestContext,
    single: SingleFieldObject,
}

#[test]
fn value_object_struct_captures_as_object_in_test_util() {
    let entry = test_util::to_test_entry(RootEntry::new(
        Metrics {
            context: RequestContext {
                request_id: "req-1",
                retries: 2,
                detail: RequestDetail {
                    shard: "blue",
                    attempt: 3,
                },
                maybe_detail: None,
            },
            single: SingleFieldObject { count: 7 },
        }
        .close(),
    ));

    let context = &entry.objects["context"];
    assert_eq!(context["request_id"], "req-1");
    assert_eq!(context["retries"], 2);
    assert_eq!(
        context["detail"].as_object().unwrap()["shard"],
        test_util::TestObjectValue::String("blue".to_string())
    );
    assert_eq!(context["detail"].as_object().unwrap()["attempt"], 3);
    assert!(!context.contains_key("maybe_detail"));
    assert_eq!(entry.objects["single"]["count"], 7);
}

#[test]
fn value_object_option_some_captures_nested_fields() {
    let entry = test_util::to_test_entry(RootEntry::new(
        Metrics {
            context: RequestContext {
                request_id: "req-2",
                retries: 1,
                detail: RequestDetail {
                    shard: "green",
                    attempt: 5,
                },
                maybe_detail: Some(RequestDetail {
                    shard: "red",
                    attempt: 1,
                }),
            },
            single: SingleFieldObject { count: 9 },
        }
        .close(),
    ));

    let context = &entry.objects["context"];
    assert_eq!(context["request_id"], "req-2");
    assert_eq!(context["retries"], 1);
    // Verify the Some path of the optional nested object
    let maybe = context["maybe_detail"].as_object().unwrap();
    assert_eq!(
        maybe["shard"],
        test_util::TestObjectValue::String("red".to_string())
    );
    assert_eq!(maybe["attempt"], 1);
}

#[test]
fn value_object_struct_emits_as_native_emf_object() {
    let mut emf = Emf::all_validations("MyApp".to_string(), vec![vec![]]);
    let mut output = Vec::new();

    emf.format(
        &RootEntry::new(
            Metrics {
                context: RequestContext {
                    request_id: "req-1",
                    retries: 2,
                    detail: RequestDetail {
                        shard: "blue",
                        attempt: 3,
                    },
                    maybe_detail: None,
                },
                single: SingleFieldObject { count: 7 },
            }
            .close(),
        ),
        &mut output,
    )
    .unwrap();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        json["context"],
        serde_json::json!({
            "request_id": "req-1",
            "retries": 2,
            "detail": {
                "shard": "blue",
                "attempt": 3,
            },
        })
    );
    assert_eq!(json["single"], serde_json::json!({ "count": 7 }));
}

// Compile test: Arc<T: ObjectValue + ?Sized> must implement ObjectValue.
// The ?Sized bound allows Arc<Box<T>> chains to implement ObjectValue.
// This would fail to compile if the Arc<T> impl was missing ?Sized.
struct ArcObjectValueCompileTest;
impl ObjectValue for ArcObjectValueCompileTest {
    fn write_object(&self, writer: &mut impl metrique::writer::value::ObjectWriter) {
        writer.field("x", &1u64);
    }
}

#[allow(dead_code)]
fn _arc_object_value_compiles() {
    let arc: StdArc<ArcObjectValueCompileTest> = StdArc::new(ArcObjectValueCompileTest);
    // Arc<Box<T>> chain — requires ?Sized on both Arc and Box impls
    let boxed: StdArc<Box<ArcObjectValueCompileTest>> =
        StdArc::new(Box::new(ArcObjectValueCompileTest));
    fn assert_object_value<T: ObjectValue>(_: &T) {}
    assert_object_value(&arc);
    assert_object_value(&boxed);
}
