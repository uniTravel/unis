use bytes::BytesMut;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

pub(crate) struct BufferPool {
    buffers: Vec<BytesMut>,
    _size: usize,
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
            _size: size,
            max_size,
        }
    }

    fn acquire(&mut self) -> BytesMut {
        self.buffers
            .pop()
            .unwrap_or_else(|| BytesMut::with_capacity(self._size))
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

pub(crate) struct BufferGuard {
    pub buf: Option<BytesMut>,
    buf_tx: mpsc::Sender<BytesMut>,
}

impl BufferGuard {
    pub fn new(inner: Arc<Mutex<BufferPool>>, buf_tx: mpsc::Sender<BytesMut>) -> Self {
        let buf = {
            let mut inner = inner.blocking_lock();
            inner.acquire()
        };

        BufferGuard {
            buf: Some(buf),
            buf_tx,
        }
    }

    pub fn into_inner(mut self) -> BytesMut {
        self.buf.take().unwrap()
    }
}

impl Drop for BufferGuard {
    fn drop(&mut self) {
        if let Some(buffer) = self.buf.take() {
            let _ = self.buf_tx.blocking_send(buffer);
        }
    }
}
