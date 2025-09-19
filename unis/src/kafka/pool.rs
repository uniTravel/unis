use crate::{errors::DomainError, kafka::subscriber::SUBSCRIBER_CONFIG};
use crossbeam::queue::ArrayQueue;
use rdkafka::{
    ClientConfig,
    consumer::{Consumer, StreamConsumer},
};
use std::sync::{Arc, LazyLock};

static CONFIG: LazyLock<ClientConfig> = LazyLock::new(|| {
    ClientConfig::new()
        .set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap)
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .set("isolation.level", "read_committed")
        .clone()
});

pub(crate) struct ConsumerPool {
    consumers: Arc<ArrayQueue<StreamConsumer>>,
}

impl ConsumerPool {
    pub(crate) fn new() -> Self {
        let consumers = Arc::new(ArrayQueue::new(SUBSCRIBER_CONFIG.aggs));

        for _ in 0..SUBSCRIBER_CONFIG.aggs {
            let consumer = CONFIG.create::<StreamConsumer>().expect("消费者创建失败");
            let _ = consumers.push(consumer);
        }

        Self { consumers }
    }

    #[inline(always)]
    pub fn get(&self) -> Result<ConsumerGuard, DomainError> {
        match self.consumers.pop() {
            Some(consumer) => Ok(ConsumerGuard {
                consumer: Some(consumer),
                pool: self.consumers.clone(),
            }),
            None => match CONFIG.create::<StreamConsumer>() {
                Ok(consumer) => Ok(ConsumerGuard {
                    consumer: Some(consumer),
                    pool: self.consumers.clone(),
                }),
                Err(e) => Err(DomainError::ReadError(e.to_string())),
            },
        }
    }
}

pub(crate) struct ConsumerGuard {
    consumer: Option<StreamConsumer>,
    pool: Arc<ArrayQueue<StreamConsumer>>,
}

impl ConsumerGuard {
    #[inline(always)]
    pub fn into_inner(mut self) -> StreamConsumer {
        self.consumer.take().unwrap()
    }
}

impl Drop for ConsumerGuard {
    fn drop(&mut self) {
        if let Some(consumer) = self.consumer.take() {
            if let Ok(_) = consumer.unassign() {
                let _ = self.pool.push(consumer);
            }
        }
    }
}
