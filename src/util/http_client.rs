use std::io;
use std::io::ErrorKind;
use std::sync::LazyLock;

use bytes::Bytes;
use futures::io::Lines;
use futures::stream::IntoAsyncRead;
use futures::stream::MapErr;
use futures::AsyncBufReadExt;
use futures::Stream;
use futures::TryStreamExt;

pub fn http_client() -> &'static reqwest::Client {
    static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);
    &HTTP_CLIENT
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
