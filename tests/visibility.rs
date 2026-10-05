use depends_rs::{depends, lifetimes};
mod model {
    use super::lifetimes;
    #[lifetimes]
    pub(crate) struct V {
        pub(crate) data: &str,
    }
}
use model::V;
#[depends(return.data = t)]
fn make(t: &str) -> V {
    V { data: t }
}
#[test]
fn v() {
    let x = String::from("x");
    assert_eq!(make(&x).data, "x");
}
