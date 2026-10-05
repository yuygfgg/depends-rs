use depends_rs::depends;
use depends_test_facade::{Container, Message};

#[depends(return.header = input, return.body = input)]
pub fn parse(input: &str) -> Message {
    Message {
        header: input,
        body: input,
    }
}

#[depends(return.prefix = prefix, return.value = input)]
pub fn wrap(prefix: &str, input: &str) -> Container<Message> {
    Container {
        prefix,
        value: parse(input),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn generic_field_paths_survive_crate_reexports() {
        let input = String::from("body");
        let message;
        {
            let prefix = String::from("prefix");
            let container = super::wrap(&prefix, &input);
            assert_eq!(container.prefix, "prefix");
            message = container.value;
        }
        assert_eq!((message.header, message.body), ("body", "body"));
    }
}
