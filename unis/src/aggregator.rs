use crate::domain::{Aggregate, Replayer, Stream};
use crate::errors::DomainError;
use crate::pool::{OneshotPool, PooledSender};
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
    time::Duration,
};
use tokio::{sync::mpsc, time};
use uuid::Uuid;

enum Task {
    Com {
        agg_id: Uuid,
        com_id: Uuid,
        com_data: Vec<u8>,
        reply_tx: PooledSender,
    },
}

pub struct Aggregator<A, R, S>
where
    A: Aggregate + Send + 'static,
    R: Replayer<A = A> + Send + 'static,
    S: Stream<A = A> + 'static,
{
    tx: mpsc::Sender<Task>,
    pool: OneshotPool,
    _marker_r: PhantomData<R>,
    _marker_s: PhantomData<S>,
}

impl<A, R, S> Aggregator<A, R, S>
where
    A: Aggregate + Send,
    R: Replayer<A = A> + Send,
    S: Stream<A = A> + Send,
{
    fn replay(replayer: &R, ds: Vec<Vec<u8>>, agg: &mut A) -> Result<(), DomainError> {
        for evt_data in ds {
            replayer.replay(agg, evt_data)?;
        }
        Ok(())
    }

    fn get_oa(
        agg_id: Uuid,
        caches: &mut HashMap<Uuid, A>,
        replayer: &R,
        stream: &S,
    ) -> Result<A, DomainError> {
        if let Some(oa) = caches.remove(&agg_id) {
            return Ok(oa);
        } else {
            let mut oa = A::new(agg_id);
            let ds = stream.read(agg_id)?;
            Self::replay(&replayer, ds, &mut oa)?;
            Ok(oa)
        }
    }

    async fn task_processor(
        mut rx: mpsc::Receiver<Task>,
        interval: Duration,
        dispatch: fn(
            agg_id: Uuid,
            com_data: Vec<u8>,
            caches: &mut HashMap<Uuid, A>,
            replayer: &R,
            stream: &S,
            get_oa: fn(Uuid, &mut HashMap<Uuid, A>, &R, &S) -> Result<A, DomainError>,
        ) -> Result<(A, A, Vec<u8>), DomainError>,
        replayer: R,
        stream: S,
    ) {
        let mut interval = time::interval(interval);
        let mut caches: HashMap<Uuid, A> = HashMap::new();
        let mut ch: HashSet<Uuid> = HashSet::new();

        loop {
            tokio::select! {
                biased;
                Some(task) = rx.recv() => {
                    match task {
                        Task::Com { agg_id, com_id, com_data, mut reply_tx } => {
                            if ch.contains(&com_id) {
                                let _ = reply_tx.send(Err(DomainError::Duplicate));
                            }
                            else {
                                match dispatch(agg_id, com_data, &mut caches, &replayer, &stream, Self::get_oa) {
                                    Ok((oa, mut na, evt_data)) =>
                                        match stream.write(agg_id, com_id, na.revision(), evt_data) {
                                            Ok(()) => {
                                                let _ = reply_tx.send(Ok(()));
                                                na.next();
                                                ch.insert(com_id);
                                                caches.insert(agg_id, na);
                                            }
                                            Err(err) => {
                                                let _ = reply_tx.send(Err(err));
                                                if oa.revision() != u64::MAX {
                                                    caches.insert(agg_id, oa);
                                                }
                                            }
                                        }
                                    Err(err) => {
                                        let _ = reply_tx.send(Err(err));
                                    },
                                }
                            }
                        },
                    }
                }
                _ = interval.tick() => {

                }
            }
        }
    }

    pub fn new(
        capacity: usize,
        interval: Duration,
        dispatch: fn(
            agg_id: Uuid,
            com_data: Vec<u8>,
            caches: &mut HashMap<Uuid, A>,
            replayer: &R,
            stream: &S,
            get_oa: fn(Uuid, &mut HashMap<Uuid, A>, &R, &S) -> Result<A, DomainError>,
        ) -> Result<(A, A, Vec<u8>), DomainError>,
        replayer: R,
        stream: S,
    ) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        tokio::spawn(Self::task_processor(
            rx, interval, dispatch, replayer, stream,
        ));
        Self {
            tx,
            pool: OneshotPool::new(capacity),
            _marker_r: PhantomData,
            _marker_s: PhantomData,
        }
    }

    pub async fn commit(
        &mut self,
        agg_id: Uuid,
        com_id: Uuid,
        com_data: Vec<u8>,
    ) -> Result<(), DomainError> {
        let (reply_tx, reply_rx) = self.pool.acquire().await;
        self.tx
            .send(Task::Com {
                agg_id,
                com_id,
                com_data,
                reply_tx,
            })
            .await
            .map_err(|_| DomainError::SendError)?;
        reply_rx.await.map_err(|_| DomainError::RecvError)?
    }
}
