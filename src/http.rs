use std::time::Duration;

use reqwest::blocking::Client;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub fn client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .user_agent(concat!("rezup/", env!("CARGO_PKG_VERSION")))
        .timeout(REQUEST_TIMEOUT)
        .build()
}
