use crate::WatchResult;

pub(crate) struct Sender<T> {
    inner: flume::Sender<T>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Sender<T> {
    pub(crate) fn new(inner: flume::Sender<T>) -> Self {
        Self {
            inner,
        }
    }
    pub(crate) async fn send(&self, msg: T) -> WatchResult<()> {
        let _ = self.inner.send_async(msg).await?;
        Ok(())
    }
}

pub struct Receiver<T> {
    inner: flume::Receiver<T>,
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}


impl<T> Receiver<T> {
    pub(crate) fn new(inner: flume::Receiver<T>) -> Self {
        Self {
            inner,
        }
    }
    pub async fn receive(&self) -> WatchResult<T> {
        Ok(self.inner.recv_async().await?)
    }
}

