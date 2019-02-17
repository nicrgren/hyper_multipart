use bytes::{Bytes, BytesMut};
use futures::{Async, Stream};
use std::str;

use crate::Error;

/// Default initial buffer capacity
pub const DEFAULT_BUFFER_CAP: usize = 35000;

pub trait MultipartResponse<T>
where
    Self: Sized,
    T: Sized,
{
    fn into_multipart_with_capacity(self, buf_cap: usize) -> Result<MultipartChunks<T>, Error>;

    fn into_multipart(self) -> Result<MultipartChunks<T>, Error> {
        self.into_multipart_with_capacity(DEFAULT_BUFFER_CAP)
    }
}

impl MultipartResponse<hyper::Body> for hyper::Response<hyper::Body> {
    fn into_multipart_with_capacity(
        self,
        buf_cap: usize,
    ) -> Result<MultipartChunks<hyper::Body>, Error> {
        let (parts, body) = self.into_parts();

        let header = parts
            .headers
            .get(http::header::CONTENT_TYPE)
            .ok_or(Error::ContentTypeMissing)
            .and_then(|header_value| header_value.to_str().map_err(Error::InvalidHeader))
            .and_then(|s| s.parse::<mime::Mime>().map_err(Error::InvalidMimeType))?;

        let boundary = header.get_param("boundary").ok_or(Error::NotMultipart)?;

        Ok(MultipartChunks::new(
            body,
            buf_cap,
            format!("\r\n--{}\r\n", boundary.as_str()),
        ))
    }
}

pub struct MultipartChunks<T> {
    inner: T,
    first_read: bool,
    buffer: BytesMut,

    // boundary
    boundary: String,
}

impl<T> MultipartChunks<T> {
    fn new(inner: T, buf_cap: usize, boundary: String) -> Self {
        Self {
            inner,
            boundary,
            first_read: false,
            buffer: BytesMut::with_capacity(buf_cap),
        }
    }
}

/// When streaming multipart. The following rules apply:
/// The boundary MUST be preceeded by CRLF which is considered to be part of the boundary;
/// e.g. given the boundary "--blockmehere" the actual boundary to find and remove from the actual
/// body is: "\r\n--blockmehere".
///
/// Additionally, directly after a boundary, there must be another CRLF, after this the
/// headers of the part arrives. After the headers there's another CRLF which indicates start of body.
/// given two CRLF after boundary, there are no headers.
///
/// Examples:
///  This is the preamble.  It is to be ignored, though it
///  is a handy place for mail composers to include an
///  explanatory note to non-MIME compliant readers.
///  --simple boundary
///
///  This is implicitly typed plain ASCII text.
///  It does NOT end with a linebreak.
///  --simple boundary
///  Content-type: text/plain; charset=us-ascii
///
///  This is explicitly typed plain ASCII text.
///  It DOES end with a linebreak.
///
///  --simple boundary--
impl Stream for MultipartChunks<hyper::Body> {
    type Item = Part;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        match self.inner.poll() {
            Ok(Async::Ready(Some(chunk))) => {
                // Chunk read. Add it to the buffer and search for the
                // separator once more.

                // Add to buffer.
                self.buffer.extend(chunk.into_bytes());

                // Search for the separator, preceeded by CRLF (already added to boundary)
                let boundary = self.boundary.as_bytes();
                match twoway::find_bytes(&self.buffer[..], boundary) {
                    Some(i) => {
                        // boundary found. take the buffer up the boundary
                        // and return it and set searched_to to 0.

                        let part_bs = if self.first_read {
                            self.buffer.split_to(i).freeze()
                        } else {
                            // Special case for the first part.
                            // The leading boundary has not been removed
                            // and does not contain the leading CRLF
                            let n_shave = self.boundary.len() - 2;
                            self.buffer.advance(n_shave);
                            self.first_read = true;
                            self.buffer.split_to(i - n_shave).freeze()
                        };

                        // shave of the boundary from the buffer.
                        self.buffer.split_to(self.boundary.as_bytes().len());

                        Ok(Async::Ready(Some(Part::from(part_bs))))
                    }

                    None => self.poll(),
                }
            }

            Ok(Async::NotReady) => {
                // debug!("Poll returning NotReady");
                Ok(Async::NotReady)
            }

            Ok(Async::Ready(None)) => {
                // debug!("Poll returning Ready(None)");
                Ok(Async::Ready(None))
            }

            Err(e) => {
                // debug!("Poll returning Error({})", e);
                Err(Error::Http(e))
            }
        }
    }
}

pub struct Part {
    // Just store the headers as the entire lines for now.
    headers_data: Bytes,
    pub body_data: Bytes,
}

impl Part {
    pub fn body(&self) -> &[u8] {
        &self.body_data
    }

    pub fn into_body(self) -> Bytes {
        self.body_data
    }

    pub fn body_len(&self) -> usize {
        self.body_data.len()
    }

    /// Returns an iterator over all the headers lines, with their line endings trimmed.
    /// Since many jpeg streams uses Headers separated by '=' instead of Https ':' this
    /// is currently the only way to get the headers.
    pub fn header_lines(&self) -> impl Iterator<Item = Result<&str, str::Utf8Error>> {
        let slice = &self.headers_data;
        slice.split(|e| *e == b'\n').map(|line| {
            // trim of the last \r
            str::from_utf8(line).map(|s| s.trim())
        })
    }
}

impl From<Bytes> for Part {
    fn from(mut bs: Bytes) -> Self {
        // split headers and body

        match twoway::find_bytes(&bs[..], b"\r\n\r\n") {
            // No headers
            None => Part {
                headers_data: Bytes::with_capacity(0),
                body_data: bs,
            },
            Some(p) => {
                let headers = bs.split_to(p);
                bs.advance(4); // remove the leading CRLF for body.
                Part {
                    headers_data: headers,
                    body_data: bs,
                }
            }
        }
    }
}
