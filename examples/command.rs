use crate::agg::{ChangeNote, NoteCommand};
use agg::CreateNote;
use bincode::config;

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

    #[derive(Debug, Encode, Decode)]
    pub struct NoteChanged {
        pub content: String,
    }

    impl Event for NoteChanged {
        type A = Note;

        fn apply(self, agg: Self::A) -> Self::A {
            Self::A {
                _content: self.content,
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

    #[derive(Debug, Validate, Encode, Decode)]
    pub struct ChangeNote {
        #[validate(length(min = 2))]
        pub content: String,
    }

    impl Command for ChangeNote {
        type A = Note;
        type E = NoteChanged;

        fn check(&self, _agg: &Self::A) -> Result<(), DomainError> {
            Ok(())
        }

        fn execute(self, _agg: &Self::A) -> Self::E {
            Self::E {
                content: self.content,
            }
        }
    }

    #[repr(u8)]
    #[derive(Debug, Encode, Decode)]
    pub enum NoteCommand {
        Create(CreateNote) = 0,
        Change(ChangeNote) = 1,
    }

    // impl Handler for NoteCommand {
    //     type A = Note;

    //     fn handle(
    //         &self,
    //         cfg_bincode: bincode::config::Configuration,
    //         com_data: Vec<u8>,
    //     ) -> Result<
    //         (
    //             Work,
    //             impl FnOnce(Self::A) -> Result<(Self::A, Vec<u8>), DomainError>,
    //         ),
    //         DomainError,
    //     > {
    //         let (com, _) = bincode::decode_from_slice(&com_data, cfg_bincode)?;
    //         match com {
    //             NoteCommand::Create(create_note) => Ok((Work::Create, |agg| Ok((agg, com_data)))),
    //             NoteCommand::Change(change_note) => {
    //                 // (Work::Apply, |agg| {
    //                 //     Ok((agg, com))
    //                 // })
    //                 todo!()
    //             }
    //         }
    //     }
    // }

    #[repr(u8)]
    #[derive(Debug, Encode, Decode)]
    pub enum NoteEvent {
        Created(NoteCreated) = 0,
        Changed(NoteChanged) = 1,
    }
}

fn main() {
    let com1 = NoteCommand::Create(CreateNote {
        title: "title".to_string(),
        content: "content".to_string(),
    });
    let com2 = NoteCommand::Change(ChangeNote {
        content: "content1".to_string(),
    });
    println!("{:#?}", com1);
    println!("{:#?}", com2);
    let config = config::standard();
    let e1 = bincode::encode_to_vec(&com1, config).unwrap();
    let e2 = bincode::encode_to_vec(&com2, config).unwrap();
    let (d1, _): (NoteCommand, _) = bincode::decode_from_slice(&e1, config).unwrap();
    println!("{:#?}", d1);
    let (d2, _): (NoteCommand, _) = bincode::decode_from_slice(&e2, config).unwrap();
    println!("{:#?}", d2);
}
