use bytes::BytesMut;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

pub(crate) struct BufferPool {
    buffers: Vec<BytesMut>,
    size: usize,
    max_size: usize,
}

impl BufferPool {
    pub fn new(capacity: usize, size: usize, max_size: usize) -> Self {
        let mut buffers = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffers.push(BytesMut::with_capacity(size));
        }

        BufferPool {
            buffers,
            size,
            max_size,
        }
    }

    fn acquire(&mut self) -> BytesMut {
        self.buffers
            .pop()
            .unwrap_or_else(|| BytesMut::with_capacity(self.size))
    }
}

pub(crate) struct BufferPoolHandler {
    pub inner: Arc<Mutex<BufferPool>>,
    pub buf_rx: mpsc::Receiver<BytesMut>,
}

impl BufferPoolHandler {
    pub async fn run(mut self) {
        while let Some(mut buf) = self.buf_rx.recv().await {
            buf.clear();
            let mut inner = self.inner.lock().await;
            if buf.capacity() <= inner.max_size {
                inner.buffers.push(buf);
            }
        }
    }
}

pub(crate) struct BufferGuard;

impl BufferGuard {
    pub fn get(inner: Arc<Mutex<BufferPool>>) -> BytesMut {
        let mut inner = inner.blocking_lock();
        inner.acquire()
    }
}
