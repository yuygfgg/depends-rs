use depends_alias::{depends, lifetimes};

#[lifetimes(crate_path = depends_alias)]
pub struct Ref {
    pub value: &str,
}

#[depends(crate_path = depends_alias, return.value = input)]
pub fn make(input: &str) -> Ref {
    Ref { value: input }
}

#[cfg(test)]
mod tests {
    use super::make;

    #[test]
    fn renamed_dependency_path_drives_metadata_queries() {
        let value = String::from("renamed");
        assert_eq!(make(&value).value, "renamed");
    }
}
