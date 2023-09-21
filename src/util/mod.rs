use std::collections::HashMap;
use std::ops::Neg;
use std::str::FromStr;
use std::sync::Mutex;

use chrono::{TimeZone, Utc};

use collections::SortedMap;

use crate::error::{Error, Result};
use crate::model::{Market, OrderBook, OrderBookUnit};
use crate::util::collections::SortedMapOrder;
use crate::{WatchError, WatchResult};

mod collections;
pub(crate) mod channel;

pub(crate) fn into_precision(s: String) -> Result<isize> {
    let d = f64::from_str(&s)?;

    if d > 1_f64 {
        return Ok(d.log10().neg() as isize);
    }

    let mut precision = 0;
    for c in s.chars() {
        if c == '0' {
            precision += 1;
        } else if c == '.' {
            precision = 0;
        } else {
            break;
        }
    }
    Ok(precision + 1isize)
}

#[cfg(test)]
mod test {
    #[test]
    fn test_into_precision() {
        assert_eq!(super::into_precision("0.00000001".to_string()).unwrap(), 8);
        assert_eq!(super::into_precision("0.001".to_string()).unwrap(), 3);
        assert_eq!(super::into_precision("10".to_string()).unwrap(), -1);
    }
}


#[derive(Debug)]
pub(crate) struct OrderBookDiff {
    first_update_id: i64,
    final_update_id: i64,
    bids: Vec<OrderBookUnit>,
    asks: Vec<OrderBookUnit>,
    timestamp: Option<i64>,
}

impl OrderBookDiff {
    pub(crate) fn new(first_update_id: i64, final_update_id: i64, bids: Vec<OrderBookUnit>, asks: Vec<OrderBookUnit>, timestamp: Option<i64>) -> Self {
        Self {
            first_update_id,
            final_update_id,
            bids,
            asks,
            timestamp,
        }
    }
}

pub(crate) fn timestamp_format(ts: i64, format: &str) -> Result<String> {
    match Utc.timestamp_millis_opt(ts) {
        chrono::LocalResult::None => Err(Error::InvalidTimestamp(ts)),
        chrono::LocalResult::Single(t) => Ok(t.format(format).to_string()),
        chrono::LocalResult::Ambiguous(_, _) => Err(Error::InvalidTimestamp(ts)),
    }
}

pub(crate) struct OrderBookAggregator {
    bids: SortedMap<String, (f64, f64)>,
    asks: SortedMap<String, (f64, f64)>,
    last_update_id: i64,
    buffer: Option<Vec<OrderBookDiff>>,
    is_synchronized: bool,
}

impl OrderBookAggregator {
    fn new() -> Self {
        Self {
            bids: SortedMap::new(SortedMapOrder::Descending),
            asks: SortedMap::new(SortedMapOrder::Ascending),
            last_update_id: 0,
            buffer: Some(Vec::new()),
            is_synchronized: false,
        }
    }


    fn snapshot(&mut self, order_book: OrderBook) -> Result<()> {
        if self.is_synchronized {
            return Err(Error::InvalidOrderBook("already synchronized".to_string()));
        }

        if self.buffer.is_none() {
            return Err(Error::InvalidOrderBook("no buffer".to_string()));
        }

        if order_book.last_update_id.is_none() {
            return Err(Error::InvalidOrderBook("no last update id".to_string()));
        }

        for unit in order_book.bids {
            handle_order_book(unit, &mut self.bids);
        }

        self.last_update_id = order_book.last_update_id.unwrap();

        for unit in order_book.asks {
            handle_order_book(unit, &mut self.asks);
        }

        for diff in self.buffer.take().unwrap() {
            if diff.final_update_id <= order_book.last_update_id.unwrap() {
                continue;
            }

            for order_book_unit in diff.bids {
                handle_order_book(order_book_unit, &mut self.bids);
            }
            for order_book_unit in diff.asks {
                handle_order_book(order_book_unit, &mut self.asks);
            }

            self.last_update_id = diff.final_update_id;
        }

        self.is_synchronized = true;
        Ok(())
    }

    fn get(&self) -> Result<OrderBook> {
        if !self.is_synchronized {
            return Err(Error::InvalidOrderBook("not synchronized".to_string()));
        }

        let best_bid = self.bids.peek().ok_or(Error::InvalidOrderBook("no bid".to_string()))?.1;
        let best_ask = self.asks.peek().ok_or(Error::InvalidOrderBook("no ask".to_string()))?.1;
        Ok(OrderBook {
            bids: vec![OrderBookUnit { price: best_bid.0, quantity: best_bid.1 }],
            asks: vec![OrderBookUnit { price: best_ask.0, quantity: best_ask.1 }],
            ..Default::default()
        })
    }

    fn append_and_get(&mut self, diff: OrderBookDiff) -> Result<Option<OrderBook>> {
        if !self.is_synchronized {
            self.buffer.as_mut().unwrap().push(diff);
            return Ok(None);
        }

        if self.is_synchronized && diff.first_update_id != self.last_update_id + 1 {
            return Err(Error::InvalidOrderBook(format!("invalid update id. diff first update id: {}, last update id: {}", diff.first_update_id, self.last_update_id)));
        }

        diff.bids.into_iter().for_each(|order_book_unit| {
            let (price, quantity) = order_book_unit.into();
            if quantity == 0_f64 {
                self.bids.remove(price.to_string());
                return;
            }
            self.bids.insert(price.to_string(), (price, quantity));
        });

        diff.asks.into_iter().for_each(|order_book_unit| {
            let (price, quantity) = order_book_unit.into();
            if quantity == 0_f64 {
                self.asks.remove(price.to_string());
                return;
            }
            self.asks.insert(price.to_string(), (price, quantity));
        });

        self.last_update_id = diff.final_update_id;

        if !self.is_synchronized {
            return Ok(None);
        }
        let best_bid = self.bids.peek().ok_or(Error::InvalidOrderBook("no bid".to_string()))?.1;
        let best_ask = self.asks.peek().ok_or(Error::InvalidOrderBook("no ask".to_string()))?.1;
        Ok(Some(OrderBook {
            bids: vec![OrderBookUnit { price: best_bid.0, quantity: best_bid.1 }],
            asks: vec![OrderBookUnit { price: best_ask.0, quantity: best_ask.1 }],
            ..Default::default()
        }))
    }
}

pub(crate) struct OrderBookSynchronizer {
    market_order_books: HashMap<Market, Mutex<OrderBookAggregator>>,
}

impl OrderBookSynchronizer {
    pub(crate) fn new() -> Self {
        Self {
            market_order_books: HashMap::new(),
        }
    }

    pub(crate) fn init(&mut self, markets: &Vec<Market>) {
        for market in markets {
            self.market_order_books.insert(market.clone(), Mutex::new(OrderBookAggregator::new()));
        }
    }

    pub(crate) fn snapshot(&self, market: Market, order_book: OrderBook) -> Result<()> {
        self.market_order_books.get(&market).ok_or(Error::InvalidMarket)?
            .lock()?
            .snapshot(order_book)
    }

    pub(crate) fn append_and_get(&self, market: Market, diff: OrderBookDiff) -> Result<Option<OrderBook>> {
        let book = self.market_order_books.get(&market).ok_or(Error::InvalidMarket)?
            .lock()?
            .append_and_get(diff)?
            .map(|mut order_book| {
                order_book.market = market;
                order_book
            });
        Ok(book)
    }

    pub(crate) fn get(&self, market: Market) -> Result<OrderBook> {
        self.market_order_books.get(&market).ok_or(Error::InvalidMarket)?
            .lock()?
            .get()
    }
}

pub(super) fn handle_order_book(order_book_unit: OrderBookUnit, order_book_hashmap: &mut SortedMap<String, (f64, f64)>) {
    let (price, quantity) = order_book_unit.into();
    if quantity == 0.0 {
        order_book_hashmap.remove(price.to_string());
    } else {
        order_book_hashmap.insert(price.to_string(), (price, quantity));
    }
}

#[cfg(test)]
mod test_order_book {
    use std::sync::{Arc, Mutex, RwLock};
    use std::thread;

    use crate::model::{Market, OrderBook, OrderBookUnit};
    use crate::util::{OrderBookAggregator, OrderBookDiff, OrderBookSynchronizer};

    #[test]
    fn test_order_book_synchronizer() {
        let sync = Arc::new(RwLock::new(OrderBookSynchronizer::new()));
        let market = Market::default();
        sync.write().unwrap().market_order_books.insert(market.clone(), Mutex::new(OrderBookAggregator::new()));

        thread::spawn({
            let mut sync = sync.clone();
            let market = market.clone();
            move || {
                thread::sleep(std::time::Duration::from_secs(5));
                let _ = sync.read()
                    .unwrap()
                    .snapshot(market.clone(),
                              OrderBook::new(vec![OrderBookUnit { price: 1.0, quantity: 1.0 }],
                                             vec![OrderBookUnit { price: 2.0, quantity: 2.0 }], market, None, Some(5)));
            }
        });

        for i in 0..10 {
            thread::sleep(std::time::Duration::from_secs(1));
            let ob = sync.read().unwrap().append_and_get(market.clone(), OrderBookDiff::new(i * 3,
                                                                                            i * 3 + 2,
                                                                                            vec![OrderBookUnit { price: i as f64, quantity: i as f64 }],
                                                                                            vec![OrderBookUnit { price: i as f64, quantity: i as f64 }],
                                                                                            None));
        }

        let order_book = sync.read().unwrap().get(market.clone()).unwrap();
        assert_eq!(order_book.bids[0].price, 9.0);
        assert_eq!(order_book.asks[0].price, 9.0);
    }
}

