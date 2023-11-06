use futures_util::StreamExt;

use crate::{WatchError, WatchResult};
use crate::client::WsClient;
use crate::exchange::StreamItem;

pub struct Receiver {
    inner: WsClient,
}


impl Receiver {
    pub(crate) fn new(inner: WsClient) -> Self {
        Self {
            inner
        }
    }
    pub async fn receive(&mut self) -> WatchResult<Option<StreamItem>> {
        let option = self.inner.next().await;
        if option.is_none() {
            return Err(WatchError::UnknownError("receive error".into()));
        }
        option.unwrap()
    }
}
