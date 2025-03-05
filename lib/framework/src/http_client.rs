use std::io;
use std::io::ErrorKind;
use std::result::Result;
use std::sync::LazyLock;
use std::time::Duration;

use bytes::Bytes;
use futures::AsyncBufReadExt;
use futures::Stream;
use futures::TryStreamExt;
use futures::io::Lines;
use futures::stream::IntoAsyncRead;
use futures::stream::MapErr;

pub static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(300))
        .connection_verbose(false)
        .build()
        .unwrap()
});

type BytesResult = Result<Bytes, reqwest::Error>;
pub trait ResponseExt {
    fn lines(
        self,
    ) -> Lines<IntoAsyncRead<MapErr<impl Stream<Item = BytesResult>, impl FnMut(reqwest::Error) -> io::Error>>>;
}

impl ResponseExt for reqwest::Response {
    fn lines(
        self,
    ) -> Lines<IntoAsyncRead<MapErr<impl Stream<Item = BytesResult>, impl FnMut(reqwest::Error) -> io::Error>>> {
        self.bytes_stream()
            .map_err(|e| io::Error::new(ErrorKind::Other, e))
            .into_async_read()
            .lines()
    }
}
