use metrique::unit_of_work::metrics;

#[metrics(value(object))]
struct RequestContext {
    request_id: &'static str,
    count: u64,
}

#[metrics(value(object))]
struct SingleFieldContext {
    count: u64,
}

#[metrics]
struct Metrics {
    context: RequestContext,
    single: SingleFieldContext,
}

#[metrics(value(object))]
struct WithIgnoredField {
    visible: u64,
    #[metrics(ignore)]
    _hidden: String,
}

fn main() {
    let _ = Metrics {
        context: RequestContext {
            request_id: "req-1",
            count: 1,
        },
        single: SingleFieldContext { count: 2 },
    };
    let _ = WithIgnoredField {
        visible: 1,
        _hidden: String::new(),
    };
}
