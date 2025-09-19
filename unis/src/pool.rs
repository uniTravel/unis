use bytes::BytesMut;
use crossbeam::queue::ArrayQueue;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct BufferPool {
    queue: Arc<ArrayQueue<BytesMut>>,
    capacity: usize,
}

impl BufferPool {
    pub fn new(capacity: usize, warm_size: usize) -> Self {
        let queue = Arc::new(ArrayQueue::new(warm_size));

        for _ in 0..warm_size {
            let _ = queue.push(BytesMut::with_capacity(capacity));
        }

        Self { queue, capacity }
    }

    #[inline(always)]
    pub fn get(&self) -> BytesMut {
        match self.queue.pop() {
            Some(mut buf) => {
                buf.clear();
                buf
            }
            None => BytesMut::with_capacity(self.capacity),
        }
    }

    #[inline(always)]
    pub fn put(&self, mut buf: BytesMut) {
        buf.clear();

        if buf.capacity() > self.capacity * 2 {
            buf = BytesMut::with_capacity(self.capacity);
        }

        let _ = self.queue.push(buf);
    }
}
