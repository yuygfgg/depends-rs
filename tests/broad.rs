use depends_rs::{depends, lifetimes};
#[lifetimes]
struct Pair {
    left: &str,
    right: &str,
}
#[depends(return = text)]
fn make(text: &str) -> Pair {
    Pair {
        left: text,
        right: text,
    }
}
#[test]
fn b() {
    let s = String::from("x");
    let p = make(&s);
    assert_eq!((p.left, p.right), ("x", "x"));
}
