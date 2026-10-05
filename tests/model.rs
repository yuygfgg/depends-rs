use depends_rs::lifetimes;

#[lifetimes]
pub struct View {
    pub data: &str,
}
