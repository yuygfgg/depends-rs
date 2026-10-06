use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

struct External<'a> {
    data: &'a str,
}

#[depends(return.data = 'a)]
fn extract<'a>(input: External<'a>) -> View {
    View { data: input.data }
}

#[depends(return.data = 'static)]
fn builtin() -> View {
    View { data: "builtin" }
}

#[depends(out.data <= 'a)]
fn update<'a>(out: &mut View, text: &'a str) {
    out.data = text;
}

#[test]
fn declared_and_static_lifetimes_can_source_relations() {
    let value = String::from("external");
    let result = extract(External { data: &value });
    assert_eq!(result.data, "external");
    assert_eq!(builtin().data, "builtin");

    let replacement = String::from("replacement");
    let mut view = View { data: "old" };
    update(&mut view, &replacement);
    assert_eq!(view.data, "replacement");
}
