use super::SUBSCRIBER_CONFIG;
use crossbeam::queue::ArrayQueue;
use rdkafka::{
    ClientConfig,
    consumer::{BaseConsumer, Consumer},
};
use std::sync::{Arc, LazyLock};
use tracing::debug;
use unis::errors::UniError;

static CONFIG: LazyLock<ClientConfig> = LazyLock::new(|| {
    ClientConfig::new()
        .set("bootstrap.servers", &SUBSCRIBER_CONFIG.bootstrap)
        .set("group.id", "agg_reader")
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .set("isolation.level", "read_committed")
        .clone()
});

pub(crate) struct ConsumerPool {
    consumers: Arc<ArrayQueue<BaseConsumer>>,
}

impl ConsumerPool {
    pub(crate) fn new() -> Self {
        let consumers = Arc::new(ArrayQueue::new(SUBSCRIBER_CONFIG.aggs));

        for _ in 0..SUBSCRIBER_CONFIG.aggs {
            let consumer = CONFIG.create::<BaseConsumer>().expect("聚合消费者创建失败");
            let _ = consumers.push(consumer);
        }

        debug!("成功创建消费者池，预热 {} 个消费者", consumers.len());
        Self { consumers }
    }

    #[inline(always)]
    pub fn get(&self) -> Result<ConsumerGuard, UniError> {
        match self.consumers.pop() {
            Some(consumer) => Ok(ConsumerGuard {
                consumer: Some(consumer),
                pool: self.consumers.clone(),
            }),
            None => match CONFIG.create::<BaseConsumer>() {
                Ok(consumer) => Ok(ConsumerGuard {
                    consumer: Some(consumer),
                    pool: self.consumers.clone(),
                }),
                Err(e) => Err(UniError::ReadError(e.to_string())),
            },
        }
    }
}

pub(crate) struct ConsumerGuard {
    consumer: Option<BaseConsumer>,
    pool: Arc<ArrayQueue<BaseConsumer>>,
}

impl ConsumerGuard {
    #[inline(always)]
    pub fn into_inner(mut self) -> BaseConsumer {
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
