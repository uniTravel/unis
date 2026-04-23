use proptest::{char, collection::vec, prelude::*};
use std::ops::Range;

pub fn digit_string(lenth: usize) -> impl Strategy<Value = String> {
    vec(b'0'..=b'9', lenth).prop_map(|bytes| String::from_utf8(bytes).unwrap())
}

pub fn short_string(le: usize) -> impl Strategy<Value = String> {
    vec(char::any(), 0..=le).prop_map(|chars| chars.into_iter().collect())
}

pub fn long_string(ge: usize) -> impl Strategy<Value = String> {
    vec(char::any(), ge..=50).prop_map(|chars| chars.into_iter().collect())
}

pub fn less_great(less: Range<i64>, great: Range<i64>) -> impl Strategy<Value = (i64, i64)> {
    if less.end <= great.start {
        return (less, great).prop_map(|(l, g)| (l, g)).boxed();
    }

    let max = less.end.min(great.end - 1);
    (less.start..max)
        .prop_flat_map(move |l| {
            let min = (l + 1).max(great.start);
            (Just(l), min..great.end)
        })
        .boxed()
}

pub fn less_great_equal(less: Range<i64>, great: Range<i64>) -> impl Strategy<Value = (i64, i64)> {
    if less.end <= great.start {
        return (less, great).prop_map(|(l, g)| (l, g)).boxed();
    }

    let max = less.end.min(great.end - 1);
    (less.start..max)
        .prop_flat_map(move |l| {
            let min = l.max(great.start);
            (Just(l), min..great.end)
        })
        .boxed()
}
