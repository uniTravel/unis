use ahash::AHashMap;
use tokio::time::Instant;
use unis::{
    BINCODE_CONFIG,
    domain::{Aggregate, Command, Event, Load, Stream},
    errors::DomainError,
};
use unis_macros::{aggregate, command, command_enum, event, event_enum};
use uuid::Uuid;

#[aggregate]
pub struct Note {
    pub title: String,
    pub content: String,
    pub grade: u32,
}

#[event]
pub struct NoteCreated {
    pub title: String,
    pub content: String,
    pub grade: u32,
}

impl Event for NoteCreated {
    type A = Note;

    fn apply(&self, agg: &mut Self::A) {
        agg.title = self.title.clone();
        agg.content = self.content.clone();
        agg.grade = self.grade;
    }
}

#[event]
pub struct NoteChanged {
    pub content: String,
}

impl Event for NoteChanged {
    type A = Note;

    fn apply(&self, agg: &mut Self::A) {
        agg.content = self.content.clone();
    }
}

#[event_enum(Note)]
pub enum NoteEvent {
    Created(NoteCreated) = 0,
    Changed(NoteChanged) = 1,
}

#[command]
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

#[command]
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

#[command_enum(Note)]
pub enum NoteCommand {
    Create(CreateNote) = 0,
    Change(ChangeNote) = 1,
}

pub fn dispatcher<S: Stream>(
    agg_type: &'static str,
    agg_id: Uuid,
    com_data: Vec<u8>,
    caches: &mut AHashMap<Uuid, (Note, Instant)>,
    loader: &impl Load<Note, Replayer, S>,
    replayer: &Replayer,
) -> Result<((Note, Instant), Note, NoteEvent), DomainError> {
    let (com, _): (NoteCommand, _) = bincode::decode_from_slice(&com_data, BINCODE_CONFIG)?;
    match com {
        NoteCommand::Create(com) => {
            let oa = Note::new(agg_id);
            let mut na = oa.clone();
            let evt = Dispatcher::<0>::new().execute(com, &mut na)?;
            Ok(((oa, Instant::now()), na, NoteEvent::Created(evt)))
        }
        NoteCommand::Change(com) => {
            let (oa, ot) = loader.load(agg_type, agg_id, caches, &replayer)?;
            let mut na = oa.clone();
            let evt = Dispatcher::<1>::new().execute(com, &mut na)?;
            Ok(((oa, ot), na, NoteEvent::Changed(evt)))
        }
    }
}
