use futures_util::stream::SelectAll;
use futures_util::StreamExt;

use crate::client::WsClient;
use crate::exchange::StreamItem;
use crate::{WatchError, WatchResult};

pub struct Receiver {
    clients: SelectAll<WsClient>,
}


impl Receiver {
    pub(crate) fn new(clients: Vec<WsClient>) -> Self {
        let clients = futures_util::stream::select_all(clients);
        Self {
            clients
        }
    }
    pub async fn receive(&mut self) -> WatchResult<StreamItem> {
        let option = self.clients.next().await;
        if option.is_none() {
            return Err(WatchError::Disconnected);
        }
        option.unwrap()
    }
}
