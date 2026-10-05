use depends_rs::lifetimes;
#[lifetimes]
struct Wrapper<T> {
    prefix: &str,
    value: T,
}
#[test]
fn g() {
    let s = "x";
    let w = Wrapper {
        prefix: s,
        value: 1,
    };
    assert_eq!((w.prefix, w.value), ("x", 1));
}
