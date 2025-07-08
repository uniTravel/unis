#[cfg(test)]
mod tests {
    use crate::domain::Aggregate;
    use unis_macros::aggregate;
    use uuid::Uuid;

    #[aggregate]
    struct Note {
        name: String,
    }

    #[test]
    fn t() {
        let mut a = Note::new(Uuid::new_v4());
        a.next();
        println!("hello {:#?}", a);
        let f = a.revision.wrapping_add(1);
        println!("{f}");
        assert_eq!(2, 2)
    }
}
