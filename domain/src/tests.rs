use proptest::{char, collection::vec, prelude::*};

pub fn proptest_config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: None,
        ..Default::default()
    }
}

pub fn digit_string(lenth: usize) -> impl Strategy<Value = String> {
    vec(b'0'..=b'9', lenth).prop_map(|bytes| String::from_utf8(bytes).unwrap())
}

pub fn short_string(le: usize) -> impl Strategy<Value = String> {
    vec(char::any(), 0..=le).prop_map(|chars| chars.into_iter().collect())
}

pub fn long_string(ge: usize) -> impl Strategy<Value = String> {
    vec(char::any(), ge..=50).prop_map(|chars| chars.into_iter().collect())
}
