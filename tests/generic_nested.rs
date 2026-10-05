use depends_rs::{depends, lifetimes};
#[lifetimes]
struct View {
    data: &str,
}
#[lifetimes]
struct Wrapper<T> {
    prefix: &str,
    value: T,
}
#[depends(return.prefix = text, return.value.data = data)]
fn make(text: &str, data: &str) -> Wrapper<View> {
    Wrapper {
        prefix: text,
        value: View { data },
    }
}
#[test]
fn n() {
    let p = String::from("p");
    let d = String::from("d");
    let w = make(&p, &d);
    assert_eq!((w.prefix, w.value.data), ("p", "d"));
}

#[lifetimes]
struct Reordered<A, B> {
    second: B,
    flag: bool,
    first: A,
}

#[depends(return.first.data = a, return.second.data = b)]
fn reordered(a: &str, b: &str) -> Reordered<View, View> {
    Reordered {
        second: View { data: b },
        flag: true,
        first: View { data: a },
    }
}

#[lifetimes]
struct Repeated<T> {
    first: T,
    rest: Vec<T>,
}

#[lifetimes]
struct Envelope<T> {
    items: Repeated<T>,
}

#[depends(return = data)]
fn repeated(data: &str) -> Envelope<View> {
    Envelope {
        items: Repeated {
            first: View { data },
            rest: vec![View { data }],
        },
    }
}

#[allow(clippy::needless_lifetimes)]
#[depends(return = input.items.rest.data)]
fn from_repeated(input: &Envelope<View>) -> &str {
    input.items.rest[0].data
}

#[depends(out.value.data <= text)]
fn set_data(out: &mut Wrapper<View>, text: &str) {
    out.value.data = text;
}

#[lifetimes]
struct Tuple<T>(&str, T);

#[depends(return.0 = prefix, return.1.data = data)]
fn tuple(prefix: &str, data: &str) -> Tuple<View> {
    Tuple(prefix, View { data })
}

#[lifetimes]
enum Either<L, R> {
    Left(L),
    Right { value: R },
}

#[depends(return.Left.0.data = left, return.Right.value.data = right)]
fn either(left: &str, right: &str, choose_left: bool) -> Either<View, View> {
    if choose_left {
        Either::Left(View { data: left })
    } else {
        Either::Right {
            value: View { data: right },
        }
    }
}

#[test]
fn paths_follow_fields_instead_of_type_parameter_order() {
    let a = String::from("a");
    let first;
    {
        let b = String::from("b");
        let value = reordered(&a, &b);
        assert_eq!(value.second.data, "b");
        assert!(value.flag);
        first = value.first;
    }
    assert_eq!(first.data, "a");
}

#[test]
fn repeated_parameters_keep_field_paths_through_nested_containers() {
    let data = String::from("data");
    let result;
    {
        let envelope = repeated(&data);
        assert_eq!(envelope.items.first.data, "data");
        result = from_repeated(&envelope);
    }
    assert_eq!(result, "data");
}

#[test]
fn generic_field_paths_support_outlives_constraints() {
    let prefix = String::from("prefix");
    let data = String::from("new");
    let mut value = make(&prefix, "old");
    set_data(&mut value, &data);
    assert_eq!((value.prefix, value.value.data), ("prefix", "new"));
}

#[test]
fn tuple_and_enum_paths_follow_declared_fields() {
    let value = tuple("prefix", "data");
    assert_eq!((value.0, value.1.data), ("prefix", "data"));
    assert!(matches!(
        either("left", "right", true),
        Either::Left(View { data: "left" })
    ));
    assert!(matches!(
        either("left", "right", false),
        Either::Right {
            value: View { data: "right" }
        }
    ));
}
