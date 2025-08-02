use crate::errors::DomainError;
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

pub trait Aggregate {
    fn new(id: Uuid) -> Self;
    fn next(&mut self);
    fn id(&self) -> Uuid;
    fn revision(&self) -> u64;
}

pub trait Event {
    type A: Aggregate;

    fn apply(&self, agg: &mut Self::A);
}

pub trait Command {
    type A: Aggregate;
    type E: Event<A = Self::A>;

    fn check(&self, agg: &Self::A) -> Result<(), DomainError>;
    fn execute(&self, agg: &Self::A) -> Self::E;
    fn process(&self, na: &mut Self::A) -> Result<Self::E, DomainError> {
        self.check(&na)?;
        let evt = self.execute(&na);
        evt.apply(na);
        Ok(evt)
    }
}

pub trait Dispatch<A, L, R, S>
where
    A: Aggregate,
    L: Load<A, R, S>,
    R: Replay<A = A>,
    S: Stream<A = A>,
{
    fn dispatch(
        &mut self,
        agg_id: Uuid,
        com_data: Vec<u8>,
        caches: &mut HashMap<Uuid, (A, OffsetDateTime)>,
        loader: &L,
        replayer: &R,
        stream: &S,
    ) -> Result<((A, OffsetDateTime), A, Vec<u8>), DomainError>;
}

pub trait Load<A, R, S>
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream<A = A>,
{
    fn load(
        &self,
        agg_id: Uuid,
        caches: &mut HashMap<Uuid, (A, OffsetDateTime)>,
        replayer: &R,
        stream: &S,
    ) -> Result<(A, OffsetDateTime), DomainError>;
}

pub trait Replay {
    type A: Aggregate;

    fn replay(&self, agg: &mut Self::A, evt_data: Vec<u8>) -> Result<(), DomainError>;
}

pub trait Stream {
    type A: Aggregate;

    fn write(
        &self,
        agg_id: Uuid,
        com_id: Uuid,
        revision: u64,
        evt_data: Vec<u8>,
    ) -> Result<(), DomainError>;
    fn read(&self, agg_id: Uuid) -> Result<Vec<Vec<u8>>, DomainError>;
}
