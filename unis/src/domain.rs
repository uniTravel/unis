use std::marker::PhantomData;

use crate::errors::DomainError;
use bincode::config::Configuration;
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
    type A: Aggregate;
    type E: Event;

    fn check(&self, agg: &Self::A) -> Result<(), DomainError>;
    fn execute(self, agg: &Self::A) -> Self::E;
}

pub enum Work<A, F>
where
    A: Aggregate,
    F: FnOnce(A) -> Result<(A, Vec<u8>), DomainError>,
{
    Create(F),
    Apply(F),
    _Marker(PhantomData<A>),
}

pub trait Handler {
    type A: Aggregate;
    type F: FnOnce(Self::A) -> Result<(Self::A, Vec<u8>), DomainError>;

    fn handle(
        &self,
        cfg_bincode: Configuration,
        com_data: Vec<u8>,
    ) -> Result<Work<Self::A, Self::F>, DomainError>;
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
