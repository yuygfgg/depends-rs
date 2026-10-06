use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

struct External<'a> {
    data: &'a str,
}

#[lifetimes]
struct Source {
    data: &str,
}

#[depends(return.data = 'a)]
fn extract<'a>(input: External<'a>) -> View {
    View { data: input.data }
}

#[depends('a <= text)]
fn make_external<'a>(text: &str) -> External<'a> {
    External { data: text }
}

#[depends('a <= text)]
fn make_external_with_shape<'a>(text: &str, _source: Source) -> External<'a> {
    let _ = _source.data;
    External { data: text }
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

#[test]
fn explicit_lifetime_targets_add_outlives_bounds() {
    let text = String::from("text");
    assert_eq!(make_external(&text).data, "text");
    assert_eq!(
        make_external_with_shape(&text, Source { data: "source" }).data,
        "text"
    );
}
