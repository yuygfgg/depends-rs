use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

trait Reader {
    #[depends(return.data = text)]
    fn read(&self, text: &str, other: &str) -> View {
        View { data: other }
    }
}

fn main() {}
