use enum_dispatch::enum_dispatch;
use uuid::Uuid;

pub trait Aggregate {
    fn new(id: Uuid) -> Self;
    fn next(&mut self);
}

#[enum_dispatch]
pub trait Event {
    type Agg: Aggregate;

    fn apply(&self, agg: &mut Self::Agg) -> Self;
}

pub trait Command {
    type Agg: Aggregate;
    type Evt: Event;

    fn validate(&self, agg: &Self::Agg) -> ();
    fn execute(&self, agg: &Self::Agg) -> ();
}

pub trait Stream {
    type Agg: Aggregate;
    fn write(&self);
}

pub trait Sender {
    type Agg: Aggregate;
}
