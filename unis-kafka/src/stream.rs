// use std::marker::PhantomData;
// use unis::domain::{self, Aggregate};

// pub struct Stream<A>
// where
//     A: Aggregate + Sync,
// {
//     _marker_a: PhantomData<A>,
// }

// impl<A> domain::Stream for Stream<A>
// where
//     A: Aggregate + Sync,
// {
//     type A = A;

//     fn write(
//         &self,
//         agg_id: uuid::Uuid,
//         com_id: uuid::Uuid,
//         revision: u64,
//         evt_data: bytes::Bytes,
//         buf_tx: tokio::sync::mpsc::Sender<bytes::BytesMut>,
//     ) -> impl Future<Output = Result<(), unis::errors::DomainError>> + Send {
//         async move { todo!() }
//     }

//     fn respond(
//         &self,
//         agg_id: uuid::Uuid,
//         com_id: uuid::Uuid,
//         res: unis::aggregator::Res,
//         evt_data: bytes::Bytes,
//     ) -> impl Future<Output = Result<(), unis::errors::DomainError>> + Send {
//         async move { todo!() }
//     }

//     fn read(&self, agg_id: uuid::Uuid) -> Result<Vec<Vec<u8>>, unis::errors::DomainError> {
//         todo!()
//     }
// }
