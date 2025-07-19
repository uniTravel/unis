use agg::Note;
use std::any::type_name;
use unis::domain::Aggregate;
use uuid::Uuid;

mod agg {
    use unis::domain::Aggregate;
    use unis_macros::aggregate;
    use uuid::Uuid;

    #[aggregate]
    pub struct Note {
        pub name: String,
    }
}

fn main() {
    let mut a = Note::new(Uuid::new_v4());
    println!("{:#?}", a);
    a.next();
    a.next();
    println!("{:#?}", a.revision());
    a.name = "hello".to_string();
    println!("{:#?}", a);
    let t = type_name::<Note>();
    println!("{:#?}", t);
}
