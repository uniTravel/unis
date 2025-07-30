use crate::domain::{Aggregate, Dispatch, Load, Replay, Stream};
use crate::errors::DomainError;
use std::{
    collections::{HashMap, VecDeque},
    marker::PhantomData,
};
use tokio::{
    sync::{mpsc, oneshot},
    time::{self, Duration},
};
use uuid::Uuid;

enum Task {
    Com {
        agg_id: Uuid,
        com_id: Uuid,
        com_data: Vec<u8>,
        reply_tx: oneshot::Sender<Result<(), DomainError>>,
    },
}

impl<A, L, R, S, F> Dispatch<A, L, R, S> for F
where
    A: Aggregate,
    L: Load<A, R, S>,
    R: Replay<A = A>,
    S: Stream<A = A>,
    F: FnMut(
        Uuid,
        Vec<u8>,
        &mut HashMap<Uuid, A>,
        &L,
        &R,
        &S,
    ) -> Result<(A, A, Vec<u8>), DomainError>,
{
    #[inline(always)]
    fn dispatch(
        &mut self,
        agg_id: Uuid,
        com_data: Vec<u8>,
        caches: &mut HashMap<Uuid, A>,
        loader: &L,
        replayer: &R,
        stream: &S,
    ) -> Result<(A, A, Vec<u8>), DomainError> {
        self(agg_id, com_data, caches, loader, replayer, stream)
    }
}

impl<A, R, S, F> Load<A, R, S> for F
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream<A = A>,
    F: Fn(Uuid, &mut HashMap<Uuid, A>, &R, &S) -> Result<A, DomainError>,
{
    #[inline(always)]
    fn load(
        &self,
        agg_id: Uuid,
        caches: &mut HashMap<Uuid, A>,
        replayer: &R,
        stream: &S,
    ) -> Result<A, DomainError> {
        self(agg_id, caches, replayer, stream)
    }
}

pub fn loader<A, R, S>(
    agg_id: Uuid,
    caches: &mut HashMap<Uuid, A>,
    replayer: &R,
    stream: &S,
) -> Result<A, DomainError>
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream<A = A>,
{
    if let Some(oa) = caches.remove(&agg_id) {
        return Ok(oa);
    } else {
        let mut oa = A::new(agg_id);
        let ds = stream.read(agg_id)?;
        for evt_data in ds {
            replayer.replay(&mut oa, evt_data)?;
        }
        Ok(oa)
    }
}

pub struct Aggregator<A, D, L, R, S>
where
    A: Aggregate,
    D: Dispatch<A, L, R, S>,
    L: Load<A, R, S>,
    R: Replay<A = A>,
    S: Stream<A = A>,
{
    tx: mpsc::Sender<Task>,
    _marker_d: PhantomData<D>,
    _marker_r: PhantomData<R>,
    _marker_s: PhantomData<S>,
    _marker_l: PhantomData<L>,
}

impl<A, D, L, R, S> Aggregator<A, D, L, R, S>
where
    A: Aggregate + Send + 'static,
    D: Dispatch<A, L, R, S> + Send + 'static,
    L: Load<A, R, S> + Send + 'static,
    R: Replay<A = A> + Send + 'static,
    S: Stream<A = A> + Send + 'static,
{
    async fn task_processor(
        mut rx: mpsc::Receiver<Task>,
        interval: Duration,
        mut dispatcher: D,
        loader: L,
        replayer: R,
        stream: S,
    ) {
        let mut interval = time::interval(interval);
        let mut caches: HashMap<Uuid, A> = HashMap::new();
        let mut ah: VecDeque<Uuid> = VecDeque::new();

        loop {
            tokio::select! {
                biased;
                Some(task) = rx.recv() => {
                    match task {
                        Task::Com { agg_id, com_id, com_data, reply_tx } => {
                            match dispatcher.dispatch(agg_id, com_data, &mut caches, &loader, &replayer, &stream) {
                                Ok((oa, mut na, evt_data)) =>
                                    match stream.write(agg_id, com_id, na.revision(), evt_data) {
                                        Ok(()) => {
                                            let _ = reply_tx.send(Ok(()));
                                            na.next();
                                            ah.push_back(agg_id);
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
                        },
                    }
                }
                _ = interval.tick() => {
                    ()
                }
            }
        }
    }

    pub async fn new(
        capacity: usize,
        interval: Duration,
        dispatcher: D,
        loader: L,
        replayer: R,
        stream: S,
    ) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        tokio::spawn(Self::task_processor(
            rx, interval, dispatcher, loader, replayer, stream,
        ));
        Self {
            tx,
            _marker_d: PhantomData,
            _marker_r: PhantomData,
            _marker_s: PhantomData,
            _marker_l: PhantomData,
        }
    }

    pub async fn commit(
        &self,
        agg_id: Uuid,
        com_id: Uuid,
        com_data: Vec<u8>,
    ) -> Result<(), DomainError> {
        let (reply_tx, reply_rx) = oneshot::channel();
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
