use super::Symbol;
use reqwest;

#[derive(Clone)]
pub struct TreasuryApiUrl(pub String);

#[derive(Serialize, Deserialize, Debug)]
pub struct RateRequestBody {
    //TODO: make it work with float
    buy_amount: u64, //ethereum
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RateResponseBody {
    pub symbol: String,
    pub rate: f64,
    pub sell_amount: u64, //satoshis
    pub buy_amount: u64,  //ethereum
}

pub trait ApiClient: Send + Sync {
    fn request_rate(
        &self,
        symbol: Symbol,
        buy_amount: u64,
    ) -> Result<RateResponseBody, reqwest::Error>;
}

#[allow(dead_code)]
pub struct DefaultApiClient {
    pub client: reqwest::Client,
    pub url: TreasuryApiUrl,
}

impl ApiClient for DefaultApiClient {
    fn request_rate(
        &self,
        symbol: Symbol,
        buy_amount: u64,
    ) -> Result<RateResponseBody, reqwest::Error> {
        self.client
            .get(format!("{}/rates/{}?amount={}", self.url.0, symbol, buy_amount).as_str())
            .send()
            .and_then(|mut res| res.json::<RateResponseBody>())
    }
}
