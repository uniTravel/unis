use crate::domain::{Aggregate, Handler, Replayer, Stream, Work};
use crate::errors::DomainError;
use crate::pool::{OneshotPool, PooledSender};
use bincode::config;
use std::collections::HashSet;
use std::{collections::HashMap, marker::PhantomData, time::Duration};
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

pub struct Aggregator<A, H, R, S>
where
    A: Aggregate + Send + Clone + 'static,
    H: Handler<A = A> + Send + 'static,
    R: Replayer<A = A> + Send + 'static,
    S: Stream<A = A> + 'static,
{
    tx: mpsc::Sender<Task>,
    pool: OneshotPool,
    _marker_h: PhantomData<H>,
    _marker_r: PhantomData<R>,
    _marker_s: PhantomData<S>,
}

impl<A, H, R, S> Aggregator<A, H, R, S>
where
    A: Aggregate + Send + Clone,
    H: Handler<A = A> + Send,
    R: Replayer<A = A> + Send,
    S: Stream<A = A> + Send,
{
    fn replay(replayer: &R, ds: Vec<Vec<u8>>, agg: &mut A) -> Result<(), DomainError> {
        for evt_data in ds {
            replayer.replay(agg, evt_data)?;
        }
        Ok(())
    }

    fn apply(
        apply: H::F,
        caches: &mut HashMap<Uuid, A>,
        ch: &mut HashSet<Uuid>,
        stream: &S,
        oa: A,
        agg_id: Uuid,
        com_id: Uuid,
        reply_tx: &mut PooledSender,
    ) {
        match apply(oa.clone()) {
            Ok((mut na, evt_data)) => match stream.write(agg_id, com_id, na.revision(), evt_data) {
                Ok(()) => {
                    let _ = reply_tx.send(Ok(()));
                    na.next();
                    ch.insert(com_id);
                    caches.insert(agg_id, na);
                }
                Err(err) => {
                    let _ = reply_tx.send(Err(err));
                    caches.insert(agg_id, oa);
                }
            },
            Err(err) => {
                let _ = reply_tx.send(Err(err));
                caches.insert(agg_id, oa);
            }
        }
    }

    async fn task_processor(
        mut rx: mpsc::Receiver<Task>,
        interval: Duration,
        handler: H,
        replayer: R,
        stream: S,
    ) {
        let mut interval = time::interval(interval);
        let mut caches: HashMap<Uuid, A> = HashMap::new();
        let mut ch: HashSet<Uuid> = HashSet::new();
        let cfg_bincode = config::standard();

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
                                match handler.handle(cfg_bincode, com_data) {
                                    Ok(work) => {
                                        match work {
                                            Work::Apply(apply) => {
                                                if let Some(oa) = caches.remove(&agg_id) {
                                                    Self::apply(apply, &mut caches, &mut ch, &stream, oa, agg_id, com_id, &mut reply_tx);
                                                }
                                                else {
                                                    let mut oa = A::new(agg_id);
                                                    match stream.read(agg_id) {
                                                        Ok(ds) => {
                                                            match Self::replay(&replayer, ds, &mut oa) {
                                                                Ok(()) => {
                                                                    Self::apply(apply, &mut caches, &mut ch, &stream, oa, agg_id, com_id, &mut reply_tx);
                                                                }
                                                                Err(err) => {
                                                                    let _ = reply_tx.send(Err(err));
                                                                }
                                                            }
                                                        },
                                                        Err(err) => {
                                                            let _ = reply_tx.send(Err(err));
                                                        },
                                                    }

                                                }
                                            },
                                            Work::Create(create) => {
                                                let agg = A::new(agg_id);
                                                match create(agg) {
                                                    Ok((mut agg, evt_data)) => {
                                                        match stream.write(agg_id, com_id, agg.revision(), evt_data){
                                                            Ok(()) => {
                                                                let _ = reply_tx.send(Ok(()));
                                                                agg.next();
                                                                ch.insert(com_id);
                                                                caches.insert(agg_id, agg);
                                                            },
                                                            Err(err) => {
                                                                let _ = reply_tx.send(Err(err));
                                                            },
                                                        }
                                                    },
                                                    Err(err) => {
                                                        let _ = reply_tx.send(Err(err));
                                                    },
                                                };
                                            },
                                            _ => unreachable!(),
                                        }
                                    }
                                    Err(err) => {
                                        let _ = reply_tx.send(Err(err));
                                    }
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

    pub fn new(capacity: usize, interval: Duration, handler: H, replay: R, stream: S) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        tokio::spawn(Self::task_processor(rx, interval, handler, replay, stream));
        Aggregator {
            tx,
            pool: OneshotPool::new(capacity),
            _marker_h: PhantomData,
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
