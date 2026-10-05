use depends_rs::lifetimes;
#[lifetimes]
struct View {
    data: &str,
}
#[lifetimes]
impl View {
    fn get(&self) -> &str {
        self.data
    }
}
#[test]
fn i() {
    let v = View { data: "x" };
    assert_eq!(v.get(), "x");
}
