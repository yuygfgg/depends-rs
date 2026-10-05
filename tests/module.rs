use depends_rs::{depends, lifetimes};
mod model {
    use super::lifetimes;
    #[lifetimes]
    pub struct View {
        pub data: &str,
    }
}
use model::View as V;
#[depends(return.data = text)]
fn make(text: &str) -> V {
    V { data: text }
}
#[test]
fn m() {
    let s = String::from("x");
    assert_eq!(make(&s).data, "x");
}
