use agg::{CreateNote, Note};
use bincode::config;
use unis::domain::{Aggregate, Command, Event};
use uuid::Uuid;
use validator::Validate;

mod agg {
    use bincode::{Decode, Encode};
    use unis::{
        domain::{Aggregate, Command, Event},
        errors::DomainError,
    };
    use unis_macros::aggregate;
    use uuid::Uuid;
    use validator::Validate;

    #[aggregate]
    pub struct Note {
        pub _title: String,
        pub _content: String,
        pub _grade: u32,
    }

    #[derive(Debug, Encode, Decode)]
    pub struct NoteCreated {
        pub title: String,
        pub content: String,
        pub grade: u32,
    }

    impl Event for NoteCreated {
        type A = Note;

        fn apply(self, agg: Self::A) -> Self::A {
            Self::A {
                _title: self.title,
                _content: self.content,
                _grade: self.grade,
                ..agg
            }
        }
    }

    #[derive(Debug, Validate, Encode, Decode)]
    pub struct CreateNote {
        #[validate(length(min = 1, max = 6))]
        pub title: String,
        #[validate(length(min = 2))]
        pub content: String,
    }

    impl Command for CreateNote {
        type A = Note;
        type E = NoteCreated;

        fn check(&self, _agg: &Self::A) -> Result<(), DomainError> {
            Ok(())
        }

        fn execute(self, _agg: &Self::A) -> Self::E {
            Self::E {
                title: self.title,
                content: self.content,
                grade: 1,
            }
        }
    }
}

fn main() {
    let note = Note::new(Uuid::new_v4());
    println!("{:#?}", note);
    let com = CreateNote {
        title: "title".to_string(),
        content: "content".to_string(),
    };
    println!("{:#?}", com);
    let config = config::standard();
    let encode = bincode::encode_to_vec(&com, config).unwrap();
    com.validate().unwrap();
    com.check(&note).unwrap();
    let evt = com.execute(&note);
    println!("{:#?}", evt);
    let (decode, _): (CreateNote, _) = bincode::decode_from_slice(&encode, config).unwrap();
    println!("{:#?}", decode);
    let note = evt.apply(note);
    println!("{:#?}", note);
}
