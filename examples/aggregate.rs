use agg::Note;
use unis::domain::Aggregate;
use uuid::Uuid;

mod agg {
    use unis::domain::Aggregate;
    use unis_macros::aggregate;
    use uuid::Uuid;

    #[aggregate(Aggregate)]
    pub struct Note {
        name: String,
    }

}

fn main() {
    let mut a = Note::new(Uuid::new_v4());
    a.next();
    println!("hello {:#?}", a);
    let f = a.revision.wrapping_add(1);
    println!("{f}");
}
