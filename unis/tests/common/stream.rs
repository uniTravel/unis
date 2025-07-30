use unis::domain::Stream;

use super::note;

pub struct UniStream;

impl Stream for UniStream {
    type A = note::Note;

    fn write(
        &self,
        _agg_id: uuid::Uuid,
        _com_id: uuid::Uuid,
        _revision: u64,
        _evt_data: Vec<u8>,
    ) -> Result<(), unis::errors::DomainError> {
        Ok(())
    }

    fn read(&self, _agg_id: uuid::Uuid) -> Result<Vec<Vec<u8>>, unis::errors::DomainError> {
        todo!()
    }
}
