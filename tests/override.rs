use depends_rs::{depends, lifetimes};
#[lifetimes]
struct Pair {
    left: &str,
    right: &str,
}
#[depends(return = a, return.right = b)]
fn make(a: &str, b: &str) -> Pair {
    Pair { left: a, right: b }
}
#[test]
fn o() {
    let a = String::from("a");
    let b = String::from("b");
    let p = make(&a, &b);
    assert_eq!((p.left, p.right), ("a", "b"));
}
