use depends_rs::lifetimes;
#[lifetimes]
struct R<'a> {
    value: &'a str,
}
#[test]
fn e() {
    let s = "x";
    let r = R { value: s };
    assert_eq!(r.value, "x");
}
