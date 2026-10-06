use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

#[lifetimes]
impl View {
    #[depends(return = self.missing)]
    fn bad(&self) -> &str {
        self.data
    }
}

fn main() {}
