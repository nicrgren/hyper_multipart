mod error;
pub use error::Error;

mod multipart;
pub use multipart::{Multipart, MultipartChunks};

mod part;
pub use part::Part;

pub mod parser;

mod header_map;
pub use header_map::HeaderMap;

use futures::Stream;

pub fn from_headers<S, I, E>(
    headers: impl HeaderMap,
    s: S,
) -> Result<impl Stream<Item = Part, Error = Error>, Error>
where
    S: Stream<Item = I, Error = E>,
    I: AsRef<[u8]>,
    E: std::fmt::Display + Send + 'static,
{
    MultipartChunks::from_parts_with_capacity(s, &headers, multipart::DEFAULT_BUFFER_CAP)
}
