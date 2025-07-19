use crate::errors::DomainError;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};

type Reply = Result<(), DomainError>;

struct OneshotPoolInner {
    senders: VecDeque<oneshot::Sender<Reply>>,
    in_use: usize,
    capacity: usize,
}

#[derive(Clone)]
pub(crate) struct OneshotPool {
    inner: Arc<Mutex<OneshotPoolInner>>,
    capacity: usize,
}

impl OneshotPool {
    pub(crate) fn new(capacity: usize) -> Self {
        let pool = Self {
            inner: Arc::new(Mutex::new(OneshotPoolInner {
                senders: VecDeque::with_capacity(capacity),
                in_use: 0,
                capacity,
            })),
            capacity,
        };
        pool.warm_up(capacity / 2);
        pool
    }

    fn warm_up(&self, count: usize) {
        let mut inner = self.inner.blocking_lock();
        let warm_up_count = count.min(self.capacity - inner.senders.len());

        for _ in 0..warm_up_count {
            let (tx, _) = oneshot::channel();
            inner.senders.push_back(tx);
        }
    }

    pub(crate) async fn acquire(&self) -> (PooledSender, oneshot::Receiver<Reply>) {
        let mut inner = self.inner.lock().await;

        let (tx, rx) = match inner.senders.pop_front() {
            Some(tx) => {
                let (_new_tx, new_rx) = oneshot::channel();
                (tx, new_rx)
            }
            None => oneshot::channel(),
        };

        inner.in_use += 1;
        (
            PooledSender {
                sender: Some(tx),
                pool: self.clone(),
            },
            rx,
        )
    }

    async fn release(&self, tx: oneshot::Sender<Reply>) {
        let mut inner = self.inner.lock().await;

        if inner.in_use > 0 {
            inner.in_use -= 1;
        }

        if inner.senders.len() < inner.capacity {
            inner.senders.push_back(tx);
        }
    }
}

pub(crate) struct PooledSender {
    sender: Option<oneshot::Sender<Reply>>,
    pool: OneshotPool,
}

impl PooledSender {
    pub(crate) fn send(&mut self, value: Reply) -> Result<(), (Reply, OneshotPool)> {
        if let Some(tx) = self.sender.take() {
            tx.send(value).map_err(|e| (e, self.pool.clone()))
        } else {
            Err((value, self.pool.clone()))
        }
    }
}

impl Drop for PooledSender {
    fn drop(&mut self) {
        if let Some(tx) = self.sender.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                pool.release(tx).await;
            });
        }
    }
}
