use metrique::unit_of_work::metrics;

#[metrics]
struct Inner {
    count: u64,
}

#[metrics(value(object), sample_group)]
struct MultiSampleGroup {
    request_id: &'static str,
    status: &'static str,
}

#[metrics(value(object))]
struct TupleValue(u64, u64);

#[metrics(value(object))]
struct FlattenedValue {
    #[metrics(flatten)]
    inner: Inner,
    count: u64,
}

#[metrics(value(object))]
enum ObjectEnum {
    A,
    B,
}

#[metrics(value(string, object))]
struct StringAndObject {
    x: u64,
}

#[metrics(value(object))]
struct WithUnit {
    #[metrics(unit = metrique::unit::Millisecond)]
    duration: u64,
}

#[metrics(value(object), rename_all = "PascalCase")]
struct WithRenameAll {
    count: u64,
}

#[metrics(value(object), prefix = "Foo_")]
struct WithPrefix {
    count: u64,
}

fn main() {}
