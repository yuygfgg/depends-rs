use depends_rs::lifetimes;
#[lifetimes]
struct Pair {
    a: &str,
    b: &str,
}
#[lifetimes]
struct Outer {
    pair: Pair,
    label: &str,
}
#[test]
fn nested() {
    let a = "a";
    let b = "b";
    let c = "c";
    let p = Pair { a, b };
    let o = Outer { pair: p, label: c };
    assert_eq!((o.pair.a, o.pair.b, o.label), ("a", "b", "c"));
}
