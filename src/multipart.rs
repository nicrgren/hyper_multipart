use crate::{
    parser::{ParseResult, Parser},
    Part,
};
use futures::{Async, Stream};
use std::error::Error as StdError;

use crate::Error;

/// Default initial buffer capacity
pub const DEFAULT_BUFFER_CAP: usize = 35000;

pub trait Multipart<T>
where
    Self: Sized,
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
        MultipartChunks::from_parts_with_capacity(body, &parts.headers, capacity)
    }
}

impl Multipart<hyper::Body> for hyper::Request<hyper::Body> {
    fn into_multipart_with_capacity(
        self,
        capacity: usize,
    ) -> Result<MultipartChunks<hyper::Body>, Error> {
        let (parts, body) = self.into_parts();

        MultipartChunks::from_parts_with_capacity(body, &parts.headers, capacity)
    }
}

impl<H, S, E, B> Multipart<S> for (H, S)
where
    Self: Sized,
    H: crate::HeaderMap,
    B: AsRef<[u8]>,
    S: Stream<Item = B, Error = E>,
    E: std::fmt::Display + Send + 'static,
{
    fn into_multipart_with_capacity(self, capacity: usize) -> Result<MultipartChunks<S>, Error> {
        let (headers, body_stream) = self;

        MultipartChunks::from_parts_with_capacity(body_stream, &headers, capacity)
    }
}

pub struct MultipartChunks<S> {
    inner: S,
    parser: Parser,
    inner_done: bool,
    inner_error: Option<Error>,
}

impl<S, E, B> MultipartChunks<S>
where
    S: Stream<Item = B, Error = E>,
    B: AsRef<[u8]>,
    E: std::fmt::Display + Send + 'static,
{
    fn from_parts_with_capacity<H: crate::HeaderMap>(
        stream: S,
        headers: &H,
        capacity: usize,
    ) -> Result<Self, Error> {
        let parser = Parser::from_with_capacity(headers, capacity)?;
        Ok(Self {
            inner: stream,
            inner_done: false,
            inner_error: None,
            parser,
        })
    }
}

impl<S, I, E> Stream for MultipartChunks<S>
where
    S: Stream<Item = I, Error = E>,
    I: AsRef<[u8]>,
    E: std::fmt::Display + Send + 'static,
{
    type Item = Part;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        let mut inner_not_ready = false;

        match self.inner.poll() {
            Ok(Async::Ready(None)) => {
                self.inner_done = true;
            }
            Err(e) => {
                self.inner_done = true;
                self.inner_error = Some(Error::inner(e));
            }

            Ok(Async::Ready(Some(chunk))) => self.parser.add_bytes(chunk),

            Ok(Async::NotReady) => inner_not_ready = true,
        }

        match self.parser.parse() {
            ParseResult::Done => Ok(Async::Ready(None)),
            ParseResult::Err(err) => Err(err.into()),
            ParseResult::Ready(bytes) => Ok(Async::Ready(Some(Part::from(bytes)))),

            ParseResult::NotReady if self.inner_done => match self.inner_error.take() {
                Some(err) => Err(err),
                None => Err(Error::malformed("Unexpected end to multipart stream")),
            },

            ParseResult::NotReady => {
                if !inner_not_ready {
                    tokio::prelude::task::current().notify()
                }

                Ok(Async::NotReady)
            }
        }
    }
}
