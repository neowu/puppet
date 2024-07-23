use std::io;
use std::io::ErrorKind;
use std::sync::OnceLock;

use bytes::Bytes;
use futures::io::Lines;
use futures::stream::IntoAsyncRead;
use futures::stream::MapErr;
use futures::AsyncBufReadExt;
use futures::Stream;
use futures::TryStreamExt;

use super::exception::Exception;

pub fn http_client() -> &'static reqwest::Client {
    static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP_CLIENT.get_or_init(reqwest::Client::new)
}

impl From<reqwest::Error> for Exception {
    fn from(err: reqwest::Error) -> Self {
        let url = err.url().map_or("", |url| url.as_str()).to_string();
        Exception::unexpected_with_context(err, &format!("url={url}"))
    }
}

pub trait ResponseExt {
    fn lines(self) -> Lines<IntoAsyncRead<MapErr<impl Stream<Item = Result<Bytes, reqwest::Error>>, impl FnMut(reqwest::Error) -> io::Error>>>;
}

impl ResponseExt for reqwest::Response {
    fn lines(self) -> Lines<IntoAsyncRead<MapErr<impl Stream<Item = Result<Bytes, reqwest::Error>>, impl FnMut(reqwest::Error) -> io::Error>>> {
        self.bytes_stream()
            .map_err(|e| io::Error::new(ErrorKind::Other, e))
            .into_async_read()
            .lines()
    }
}
