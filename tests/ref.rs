use depends_rs::depends;
#[allow(clippy::needless_lifetimes)]
#[depends(return = text)]
fn id(text: &str) -> &str {
    text
}
#[test]
fn r() {
    let s = String::from("x");
    assert_eq!(id(&s), "x");
}
