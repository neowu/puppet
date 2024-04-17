use std::sync::OnceLock;

use super::exception::Exception;

pub fn http_client() -> &'static reqwest::Client {
    static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP_CLIENT.get_or_init(reqwest::Client::new)
}

impl From<reqwest::Error> for Exception {
    fn from(err: reqwest::Error) -> Self {
        let url = err.url().map_or("", |url| url.as_str()).to_string();
        Exception::from_with_context(err, format!("url={}", url))
    }
}
