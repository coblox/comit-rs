use bitcoin_rpc_client::TransactionId;
use failure::Error;
use futures_ext::{PollUntilReady, StreamTemplate};
use ledger_query_service::{BitcoinQuery, LedgerQueryServiceApiClient, QueryId};
use std::{sync::Arc, time::Duration};
use swap_protocols::ledger::bitcoin::Bitcoin;
use tokio::prelude::*;

#[derive(Clone)]
pub struct LedgerServices {
    api_client: Arc<LedgerQueryServiceApiClient<Bitcoin, BitcoinQuery>>,
    bitcoin_poll_interval: Duration,
}

impl LedgerServices {
    pub fn new(
        api_client: Arc<LedgerQueryServiceApiClient<Bitcoin, BitcoinQuery>>,
        bitcoin_poll_interval: Duration,
    ) -> LedgerServices {
        LedgerServices {
            api_client,
            bitcoin_poll_interval,
        }
    }
}

pub struct PaymentsToBitcoinAddressStream<F> {
    inner: F,
    transactions: Vec<TransactionId>,
    next_index: usize,
}

impl<F: Future<Item = Vec<TransactionId>, Error = Error>> Stream
    for PaymentsToBitcoinAddressStream<F>
{
    type Item = TransactionId;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Option<<Self as Stream>::Item>>, <Self as Stream>::Error> {
        if let Some(transaction) = self.transactions.get(self.next_index) {
            self.next_index += 1;
            return Ok(Async::Ready(Some(transaction.clone())));
        }

        let inner_result = self.inner.poll();

        match inner_result {
            Ok(Async::Ready(transactions)) => {
                self.transactions = transactions;
                self.poll()
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}

pub struct FetchBitcoinQueryResultsFuture {
    query_id: QueryId<Bitcoin>,
    api_client: Arc<LedgerQueryServiceApiClient<Bitcoin, BitcoinQuery>>,
}

impl Future for FetchBitcoinQueryResultsFuture {
    type Item = Vec<TransactionId>;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<<Self as Future>::Item>, <Self as Future>::Error> {
        self.api_client
            .fetch_results(&self.query_id)
            .into_future()
            .poll()
    }
}

impl StreamTemplate<LedgerServices> for QueryId<Bitcoin> {
    type Stream = PaymentsToBitcoinAddressStream<PollUntilReady<FetchBitcoinQueryResultsFuture>>;

    fn into_stream(
        self,
        dependencies: LedgerServices,
    ) -> PaymentsToBitcoinAddressStream<PollUntilReady<FetchBitcoinQueryResultsFuture>> {
        PaymentsToBitcoinAddressStream {
            inner: {
                PollUntilReady::new(
                    FetchBitcoinQueryResultsFuture {
                        query_id: self,
                        api_client: dependencies.api_client,
                    },
                    dependencies.bitcoin_poll_interval,
                )
            },
            transactions: Vec::new(),
            next_index: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_ext::FutureFactory;
    use spectral::prelude::*;
    use std::sync::Mutex;
    use tokio::runtime::Runtime;

    struct FakeLedgerQueryService {
        number_of_invocations_before_result: u32,
        invocations: Mutex<u32>,
        results: Vec<TransactionId>,
    }

    impl LedgerQueryServiceApiClient<Bitcoin, BitcoinQuery> for FakeLedgerQueryService {
        fn create(&self, _query: BitcoinQuery) -> Result<QueryId<Bitcoin>, Error> {
            Ok(QueryId::new("http://localhost/results/1".parse().unwrap()))
        }

        fn fetch_results(&self, _query: &QueryId<Bitcoin>) -> Result<Vec<TransactionId>, Error> {
            let mut invocations = self.invocations.lock().unwrap();

            *invocations += 1;

            if *invocations >= self.number_of_invocations_before_result {
                Ok(self.results.clone())
            } else {
                Ok(Vec::new())
            }
        }

        fn delete(&self, _query: &QueryId<Bitcoin>) {
            unimplemented!()
        }
    }

    #[test]
    fn given_future_resolves_to_transaction_eventually() {
        let ledger_query_service = Arc::new(FakeLedgerQueryService {
            number_of_invocations_before_result: 5,
            invocations: Mutex::new(0),
            results: vec![
                "7e7c52b1f46e7ea2511e885d8c0e5df9297f65b6fff6907ceb1377d0582e45f4"
                    .parse()
                    .unwrap(),
            ],
        });

        let future_factory = FutureFactory::new(LedgerServices::new(
            ledger_query_service.clone(),
            Duration::from_millis(100),
        ));

        let stream = future_factory.create_stream_from_template(QueryId::new(
            "http://localhost/results/1".parse().unwrap(),
        ));

        let mut _runtime = Runtime::new().unwrap();

        let result = _runtime.block_on(stream.into_future());
        let result = result.map(|(item, _stream)| item).map_err(|(e1, _e2)| e1);

        let invocations = ledger_query_service.invocations.lock().unwrap();

        assert_that(&*invocations).is_equal_to(5);
        assert_that(&result).is_ok().is_some();
    }
}
