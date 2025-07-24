use crate::{BINCODE_CONFIG, errors::DomainError};
use bincode::Encode;
use uuid::Uuid;

pub trait Aggregate {
    fn new(id: Uuid) -> Self;
    fn next(&mut self);
    fn id(&self) -> Uuid;
    fn revision(&self) -> u64;
}

pub trait Event {
    type A: Aggregate;

    fn apply(self, agg: Self::A) -> Self::A;
}

pub trait Command {
    type A: Aggregate + Clone;
    type E: Event<A = Self::A> + Encode;

    fn check(&self, agg: &Self::A) -> Result<(), DomainError>;
    fn execute(&self, agg: &Self::A) -> Self::E;
    fn process(&self, oa: Self::A) -> Result<(Self::A, Self::A, Vec<u8>), DomainError> {
        self.check(&oa)?;
        let evt = self.execute(&oa);
        let evt_data = bincode::encode_to_vec(&evt, BINCODE_CONFIG)?;
        let na = evt.apply(oa.clone());
        Ok((oa, na, evt_data))
    }
}

pub trait Replayer {
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
