//! 聚合器
//!
//!

use crate::config::AggConfig;
use crate::domain::{Aggregate, Dispatch, Load, Replay, Stream};
use crate::errors::DomainError;
use std::{collections::HashMap, marker::PhantomData, sync::Arc};
use time::OffsetDateTime;
use tokio::{
    sync::{Semaphore, mpsc, oneshot},
    time::Duration,
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
        &mut HashMap<Uuid, (A, OffsetDateTime)>,
        &L,
        &R,
        &S,
    ) -> Result<((A, OffsetDateTime), A, Vec<u8>), DomainError>,
{
    #[inline(always)]
    fn dispatch(
        &mut self,
        agg_id: Uuid,
        com_data: Vec<u8>,
        caches: &mut HashMap<Uuid, (A, OffsetDateTime)>,
        loader: &L,
        replayer: &R,
        stream: &S,
    ) -> Result<((A, OffsetDateTime), A, Vec<u8>), DomainError> {
        self(agg_id, com_data, caches, loader, replayer, stream)
    }
}

impl<A, R, S, F> Load<A, R, S> for F
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream<A = A>,
    F: Fn(
        Uuid,
        &mut HashMap<Uuid, (A, OffsetDateTime)>,
        &R,
        &S,
    ) -> Result<(A, OffsetDateTime), DomainError>,
{
    #[inline(always)]
    fn load(
        &self,
        agg_id: Uuid,
        caches: &mut HashMap<Uuid, (A, OffsetDateTime)>,
        replayer: &R,
        stream: &S,
    ) -> Result<(A, OffsetDateTime), DomainError> {
        self(agg_id, caches, replayer, stream)
    }
}

/// 泛型的聚合加载函数
///
/// 构造聚合器时，具体化的聚合加载函数作为参数，系统会自动将其转为闭包传入。
pub fn loader<A, R, S>(
    agg_id: Uuid,
    caches: &mut HashMap<Uuid, (A, OffsetDateTime)>,
    replayer: &R,
    stream: &S,
) -> Result<(A, OffsetDateTime), DomainError>
where
    A: Aggregate,
    R: Replay<A = A>,
    S: Stream<A = A>,
{
    if let Some(o) = caches.remove(&agg_id) {
        return Ok(o);
    } else {
        let mut oa = A::new(agg_id);
        let ds = stream.read(agg_id)?;
        for evt_data in ds {
            replayer.replay(&mut oa, evt_data)?;
        }
        Ok((oa, OffsetDateTime::now_utc()))
    }
}

/// 聚合器
pub struct Aggregator<A, D, L, R, S>
where
    A: Aggregate,
    D: Dispatch<A, L, R, S>,
    L: Load<A, R, S>,
    R: Replay<A = A>,
    S: Stream<A = A>,
{
    tx: mpsc::Sender<Task>,
    semaphore: Arc<Semaphore>,
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
        cfg: AggConfig,
        mut rx: mpsc::Receiver<Task>,
        mut dispatcher: D,
        loader: L,
        replayer: R,
        stream: S,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(cfg.interval));
        let mut caches: HashMap<Uuid, (A, OffsetDateTime)> = HashMap::with_capacity(cfg.high);

        loop {
            tokio::select! {
                biased;
                Some(task) = rx.recv() => {
                    match task {
                        Task::Com { agg_id, com_id, com_data, reply_tx } => {
                            match dispatcher.dispatch(agg_id, com_data, &mut caches, &loader, &replayer, &stream) {
                                Ok(((oa, ot), mut na, evt_data)) =>
                                    match stream.write(agg_id, com_id, na.revision(), evt_data) {
                                        Ok(()) => {
                                            let _ = reply_tx.send(Ok(()));
                                            na.next();
                                            caches.insert(agg_id, (na, OffsetDateTime::now_utc()));
                                        }
                                        Err(err) => {
                                            let _ = reply_tx.send(Err(err));
                                            if oa.revision() != u64::MAX {
                                                caches.insert(agg_id, (oa, ot));
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
                    match caches.len() {
                        len if len <= cfg.low => (),
                        len if len > cfg.high => {
                            let mut retain = cfg.retain;
                            let mut baseline = OffsetDateTime::now_utc() - Duration::from_secs(retain);
                            caches.retain(|_, (_, t)| *t > baseline);
                            while caches.len() > cfg.high {
                                retain = retain / 2;
                                baseline = baseline + Duration::from_secs(retain);
                                caches.retain(|_, (_, t)| *t > baseline);
                            }
                        },
                        _ => {
                            let baseline = OffsetDateTime::now_utc() - Duration::from_secs(cfg.retain);
                            caches.retain(|_, (_, t)| *t > baseline);
                        }
                    }
                }
            }
        }
    }

    /// 聚合器构造函数
    pub async fn new(cfg: AggConfig, dispatcher: D, loader: L, replayer: R, stream: S) -> Self {
        let (tx, rx) = mpsc::channel(cfg.capacity);
        let semaphore = Arc::new(Semaphore::new(cfg.capacity));
        tokio::spawn(Self::task_processor(
            cfg, rx, dispatcher, loader, replayer, stream,
        ));
        Self {
            tx,
            semaphore,
            _marker_d: PhantomData,
            _marker_r: PhantomData,
            _marker_s: PhantomData,
            _marker_l: PhantomData,
        }
    }

    /// 提交命令的函数
    pub async fn commit(
        &self,
        agg_id: Uuid,
        com_id: Uuid,
        com_data: Vec<u8>,
    ) -> Result<(), DomainError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _permit = self.semaphore.acquire().await.unwrap();
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
