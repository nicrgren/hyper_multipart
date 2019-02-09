use bytes::{Bytes, BytesMut};
use futures::{Async, Stream};
use log::debug;
use std::io::{self, BufRead};

use crate::Error;

pub trait MultipartResponse<T>
where
    Self: Sized,
    T: Sized,
{
    fn multipart(self) -> Result<MultipartChunks<T>, Error>;
}

impl MultipartResponse<hyper::Body> for hyper::Response<hyper::Body> {
    fn multipart(self) -> Result<MultipartChunks<hyper::Body>, Error> {
        let (parts, body) = self.into_parts();

        let ct: mime::Mime = match parts
            .headers
            .get("content-type")
            .map(|s| s.to_str().unwrap().parse::<mime::Mime>())
        {
            Some(Ok(ct)) => ct,
            Some(Err(e)) => return Err(Error::Custom(format!("Content-Type, invalid MIME: {}", e))),
            None => return Err(Error::Custom("Content-Type header missing".to_string())),
        };

        // Parse ct to make sure it's multipart and that the boundary is given.
        // In the future, we might want to handle mixed differently then others.
        // But for now, we just chunk all bytes between seps.

        let boundary = ct.get_param("boundary").expect("Boundary not set");
        let boundary = format!("\r\n--{}\r\n", boundary.as_str());

        Ok(MultipartChunks::new(body, boundary))
    }
}

pub struct MultipartChunks<T> {
    inner: T,

    // special case for the first part
    first_read: bool,

    // This should probably be done using a BytesMut, but I cant figure out
    // how to do it properly...
    // All writes advance the cursor meaning all searches are done from the
    // written stuffs.
    // Implementing this properly using BytesMut is left as an excersise
    // for a rainy day.
    buffer: BytesMut,

    // boundary
    boundary: String,
    // The last index we searched up until.
    // Previously we stored the index up to which we previously searched.
    // Doing this fucked something up.
    // Right now we currently search from the beginning everytime we receive new data
    // TODO(nicrgren): Optimize this bs.
}

impl<T> MultipartChunks<T> {
    fn new(inner: T, boundary: String) -> Self {
        Self {
            inner: inner,
            first_read: false,
            buffer: BytesMut::with_capacity(50000),
            boundary: boundary,
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

                        // extract the body.

                        // shave of the boundary from the buffer.
                        self.buffer.split_to(self.boundary.as_bytes().len());

                        let part = Part::try_from(part_bs).unwrap();

                        Ok(Async::Ready(Some(part)))
                    }

                    None => {
                        // boundary not found. set last_searched to the length of current
                        // buffer - the boundary length.
                        // TODO(nicrgren):
                        // If a super tiny part is read, this might overflow. Fix with
                        // a simply if case.

                        self.poll()
                    }
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
    pub headers: Vec<String>,
    pub body: Bytes,
}

impl Part {
    fn try_from(mut bs: Bytes) -> Result<Self, Error> {
        // parse this multipart blob.
        let mut line = String::new();
        let mut c = io::Cursor::new(&bs);
        let mut headers = Vec::new();

        // Read headers..

        while let Ok(_read) = c.read_line(&mut line) {
            match line.trim() {
                "" => break,
                s => {
                    debug!("Read header: {}", s);
                    headers.push(s.to_string())
                }
            }

            line.clear();
        }

        let c_position = c.position() as usize;
        bs.advance(c_position);
        debug!("Body size: {}", bs.len());
        Ok(Part {
            headers: headers,
            body: bs,
        })
    }
}
