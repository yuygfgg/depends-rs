use depends_rs::depends;
mod model;
use model::View;

#[depends(return.data = text)]
fn make(text: &str) -> View {
    View { data: text }
}

#[test]
fn cross_file() {
    let value = String::from("file");
    assert_eq!(make(&value).data, "file");
}
