use crate::{
    parser::{ParseResult, Parser},
    Part,
};
use futures::{Async, Stream};
use http::header::HeaderMap;

use crate::Error;

/// Default initial buffer capacity
pub const DEFAULT_BUFFER_CAP: usize = 35000;

pub trait Multipart<T>
where
    Self: Sized,
    T: Sized,
{
    fn into_multipart_with_capacity(self, buf_cap: usize) -> Result<MultipartChunks<T>, Error>;

    fn into_multipart(self) -> Result<MultipartChunks<T>, Error> {
        self.into_multipart_with_capacity(DEFAULT_BUFFER_CAP)
    }
}

impl Multipart<hyper::Body> for hyper::Response<hyper::Body> {
    fn into_multipart_with_capacity(
        self,
        capacity: usize,
    ) -> Result<MultipartChunks<hyper::Body>, Error> {
        let (parts, body) = self.into_parts();
        MultipartChunks::from_parts_with_capacity(body, parts.headers, capacity)
    }
}

impl Multipart<hyper::Body> for hyper::Request<hyper::Body> {
    fn into_multipart_with_capacity(
        self,
        capacity: usize,
    ) -> Result<MultipartChunks<hyper::Body>, Error> {
        let (parts, body) = self.into_parts();

        MultipartChunks::from_parts_with_capacity(body, parts.headers, capacity)
    }
}

pub struct MultipartChunks<S> {
    inner: S,
    parser: Parser,
}

impl<S, E> MultipartChunks<S>
where
    S: Stream<Item = hyper::body::Chunk, Error = E>,
    E: Into<Error>,
{
    fn from_parts_with_capacity(
        stream: S,
        headers: HeaderMap,
        capacity: usize,
    ) -> Result<Self, Error> {
        let parser = Parser::from_with_capacity(headers, capacity)?;
        Ok(Self {
            inner: stream,
            parser,
        })
    }
}

impl<S, E> Stream for MultipartChunks<S>
where
    S: Stream<Item = hyper::body::Chunk, Error = E>,
    E: Into<Error>,
{
    type Item = Part;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        match self.inner.poll() {
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e.into()),

            Ok(Async::Ready(Some(chunk))) => match self.parser.parse(chunk) {
                ParseResult::Done => Ok(Async::Ready(None)),
                ParseResult::NotReady => self.poll(),
                ParseResult::Err(err) => Err(err.into()),
                ParseResult::Ready(bytes) => Ok(Async::Ready(Some(Part::from(bytes)))),
            },
        }
    }
}
