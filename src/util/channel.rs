use crate::WatchResult;

pub(crate) struct Sender<T> {
    inner: async_broadcast::Sender<T>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Clone> Sender<T> {
    pub(crate) fn new(mut inner: async_broadcast::Sender<T>) -> Self {
        inner.set_overflow(true);
        Self {
            inner,
        }
    }
    pub(crate) async fn send(&self, msg: T) -> WatchResult<()> {
        let _ = self.inner.broadcast(msg).await?;
        Ok(())
    }
}

pub struct Receiver<T> {
    inner: async_broadcast::Receiver<T>,
}

impl<T: Clone> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.new_receiver(),
        }
    }
}


impl<T: Clone> Receiver<T> {
    pub(crate) fn new(inner: async_broadcast::Receiver<T>) -> Self {
        Self {
            inner
        }
    }
    pub async fn receive(&mut self) -> WatchResult<T> {
        Ok(self.inner.recv().await?)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

