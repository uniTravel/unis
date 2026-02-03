use unis::{
    domain::{Command, CommandEnum, Event, Load},
    errors::UniError,
    macros::*,
};
use uuid::Uuid;

#[aggregate]
pub struct Note {
    title: String,
    content: String,
    grade: u32,
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

    fn check(&self, _agg: &Self::A) -> Result<(), UniError> {
        Ok(())
    }

    fn apply(self, _agg: &Self::A) -> Self::E {
        Self::E {
            title: self.title,
            content: self.content,
            grade: 1,
        }
    }
}

#[event]
pub struct NoteCreated {
    title: String,
    content: String,
    grade: u32,
}

impl Event for NoteCreated {
    type A = Note;

    fn apply(&self, agg: &mut Self::A) {
        agg.title = self.title.clone();
        agg.content = self.content.clone();
        agg.grade = self.grade;
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

    fn check(&self, _agg: &Self::A) -> Result<(), UniError> {
        Ok(())
    }

    fn apply(self, _agg: &Self::A) -> Self::E {
        Self::E {
            content: self.content,
        }
    }
}

#[event]
pub struct NoteChanged {
    content: String,
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

#[command_enum]
pub enum NoteCommand {
    Create(CreateNote) = 0,
    Change(ChangeNote) = 1,
}

impl CommandEnum for NoteCommand {
    type A = Note;
    type E = NoteEvent;

    async fn apply(
        self,
        agg_type: &'static str,
        agg_id: Uuid,
        mut agg: Self::A,
        loader: impl Load<Self::E>,
    ) -> Result<(Note, NoteEvent), UniError> {
        match self {
            NoteCommand::Create(com) => {
                let evt = com.process(&mut agg)?;
                Ok((agg, NoteEvent::Created(evt)))
            }
            NoteCommand::Change(com) => {
                replay(agg_type, agg_id, &mut agg, loader).await?;
                let evt = com.process(&mut agg)?;
                Ok((agg, NoteEvent::Changed(evt)))
            }
        }
    }
}
