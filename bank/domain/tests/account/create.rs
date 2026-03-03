use proptest::prelude::*;

fn _valid_code() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::char::range('0', '9'), 6).prop_map(|chars| chars.iter().collect())
}
