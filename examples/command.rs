use crate::agg::{ChangeNote, NoteCommand};
use agg::CreateNote;
use unis::BINCODE_CONFIG;

mod agg {
    use bincode::{Decode, Encode};
    use std::collections::HashMap;
    use unis::{
        BINCODE_CONFIG,
        domain::{Aggregate, Command, Event, Replayer, Stream},
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

        fn execute(&self, _agg: &Self::A) -> Self::E {
            Self::E {
                title: self.title.clone(),
                content: self.content.clone(),
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

        fn execute(&self, _agg: &Self::A) -> Self::E {
            Self::E {
                content: self.content.clone(),
            }
        }
    }

    #[repr(u8)]
    #[derive(Debug, Encode, Decode)]
    pub enum NoteCommand {
        Create(CreateNote) = 0,
        Change(ChangeNote) = 1,
    }

    struct _Dispatcher<const ID: usize> {}

    impl<const ID: usize> _Dispatcher<ID> {
        const fn _new() -> Self {
            Self {}
        }

        #[inline(always)]
        fn _execute<C, E>(&self, com: C, agg: Note) -> Result<(Note, Note, Vec<u8>), DomainError>
        where
            C: Command<A = Note, E = E>,
            E: Event<A = Note>,
        {
            match ID {
                0 => com.process(agg),
                1 => com.process(agg),
                _ => unsafe { std::hint::unreachable_unchecked() },
            }
        }
    }

    pub(crate) fn _dispatch<R, S>(
        agg_id: Uuid,
        com_data: Vec<u8>,
        caches: &mut HashMap<Uuid, Note>,
        replayer: &R,
        stream: &S,
        get_oa: fn(Uuid, &mut HashMap<Uuid, Note>, &R, &S) -> Result<Note, DomainError>,
    ) -> Result<(Note, Note, Vec<u8>), DomainError>
    where
        R: Replayer<A = Note> + Send + 'static,
        S: Stream<A = Note> + Send + 'static,
    {
        let (com, _): (NoteCommand, _) = bincode::decode_from_slice(&com_data, BINCODE_CONFIG)?;
        match com {
            NoteCommand::Create(com) => _Dispatcher::<0>::_new()._execute(com, Note::new(agg_id)),
            NoteCommand::Change(com) => {
                _Dispatcher::<1>::_new()._execute(com, get_oa(agg_id, caches, replayer, stream)?)
            }
        }
    }

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
    let e1 = bincode::encode_to_vec(&com1, BINCODE_CONFIG).unwrap();
    let e2 = bincode::encode_to_vec(&com2, BINCODE_CONFIG).unwrap();
    let (d1, _): (NoteCommand, _) = bincode::decode_from_slice(&e1, BINCODE_CONFIG).unwrap();
    println!("{:#?}", d1);
    let (d2, _): (NoteCommand, _) = bincode::decode_from_slice(&e2, BINCODE_CONFIG).unwrap();
    println!("{:#?}", d2);
}
