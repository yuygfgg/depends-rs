use depends_rs::{depends, lifetimes};

mod left {
    use depends_rs::lifetimes;
    #[lifetimes]
    pub struct View {
        pub data: &str,
    }
}
mod right {
    use depends_rs::lifetimes;
    #[lifetimes]
    pub struct View {
        pub first: &str,
        pub second: &str,
    }
}
mod facade {
    pub use super::left::*;
}

#[lifetimes]
struct Both {
    left: left::View,
    right: right::View,
}
#[depends(return = a, return.right.second = b)]
fn qualified(a: &str, b: &str) -> Both {
    Both {
        left: left::View { data: a },
        right: right::View {
            first: a,
            second: b,
        },
    }
}
#[depends(return.data = text)]
fn glob_import(text: &str) -> facade::View {
    facade::View { data: text }
}
#[depends(return.data = text)]
fn fully_qualified(text: &str) -> crate::left::View {
    left::View { data: text }
}

#[lifetimes]
struct Wrapper<T> {
    prefix: &str,
    value: T,
}
#[lifetimes]
impl<T> Wrapper<T> {
    fn prefix(&self) -> &str {
        self.prefix
    }
    fn value(&self) -> &T {
        &self.value
    }
}
#[lifetimes]
struct Nested {
    wrapper: Wrapper<left::View>,
}
#[depends(return.wrapper.prefix = text, return.wrapper.value.data = data)]
fn generic_nested(text: &str, data: &str) -> Nested {
    Nested {
        wrapper: Wrapper {
            prefix: text,
            value: left::View { data },
        },
    }
}

#[lifetimes]
enum Selection<T> {
    Left { value: &str, extra: T },
    Right { value: &str },
    Pair(&str, &str),
}
#[depends(return = a, return.Right.value = b, return.Pair.1 = b)]
fn select(a: &str, b: &str, index: u8) -> Selection<u8> {
    match index {
        0 => Selection::Left { value: a, extra: 7 },
        1 => Selection::Right { value: b },
        _ => Selection::Pair(a, b),
    }
}

#[lifetimes]
struct Tuple(&str, &str);
#[depends(return.0 = a, return.1 = b)]
fn tuple(a: &str, b: &str) -> Tuple {
    Tuple(a, b)
}

#[depends(return.data = text)]
fn transition(_old: left::View, text: &str) -> left::View {
    left::View { data: text }
}

#[test]
fn full_paths_and_reexports_keep_distinct_shapes() {
    let a = String::from("a");
    let b = String::from("b");
    let both = qualified(&a, &b);
    assert_eq!(
        (both.left.data, both.right.first, both.right.second),
        ("a", "a", "b")
    );
    assert_eq!(glob_import(&a).data, "a");
    assert_eq!(fully_qualified(&b).data, "b");
}

#[test]
fn generic_arguments_and_impl_keep_their_type_parameters() {
    let text = String::from("prefix");
    let data = String::from("data");
    let nested = generic_nested(&text, &data);
    assert_eq!(nested.wrapper.prefix(), "prefix");
    assert_eq!(nested.wrapper.value().data, "data");
}

#[test]
fn enum_paths_include_the_variant_and_tuple_index() {
    assert!(matches!(
        select("a", "b", 0),
        Selection::Left {
            value: "a",
            extra: 7
        }
    ));
    assert!(matches!(
        select("a", "b", 1),
        Selection::Right { value: "b" }
    ));
    assert!(matches!(select("a", "b", 2), Selection::Pair("a", "b")));
    let tuple = tuple("a", "b");
    assert_eq!((tuple.0, tuple.1), ("a", "b"));
}

#[test]
fn consuming_transition_releases_the_old_borrow() {
    let result;
    let new = String::from("new");
    {
        let old = String::from("old");
        result = transition(left::View { data: &old }, &new);
    }
    assert_eq!(result.data, "new");
}
