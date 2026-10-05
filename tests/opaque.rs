use depends_rs::lifetimes;
struct ExternalType;
#[lifetimes(opaque(ExternalType))]
struct Holder {
    value: ExternalType,
    text: &str,
}
#[test]
fn o() {
    let h = Holder {
        value: ExternalType,
        text: "x",
    };
    let Holder { value, text } = h;
    assert!(matches!(value, ExternalType));
    assert_eq!(text, "x");
}
