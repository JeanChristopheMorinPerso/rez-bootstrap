use std::time::Duration;

use reqwest::blocking::Client as BlockingClient;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub fn client() -> Result<BlockingClient, reqwest::Error> {
    BlockingClient::builder()
        .user_agent(concat!("rezup/", env!("CARGO_PKG_VERSION")))
        .timeout(REQUEST_TIMEOUT)
        .build()
}

pub fn async_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .user_agent(concat!("rezup/", env!("CARGO_PKG_VERSION")))
        .timeout(REQUEST_TIMEOUT)
        .build()
}
