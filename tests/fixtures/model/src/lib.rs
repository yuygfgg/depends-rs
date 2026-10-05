use depends_rs::lifetimes;

#[lifetimes]
pub struct Packet {
    pub header: &str,
    pub body: &str,
}

#[lifetimes]
pub struct Wrapper<T> {
    pub prefix: &str,
    pub value: T,
}
