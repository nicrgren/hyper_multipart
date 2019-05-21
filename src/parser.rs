use crate::Error;
use bytes::{Bytes, BytesMut};

#[derive(Debug)]
pub(crate) enum ParseResult {
    Done,
    NotReady,
    Ready(Bytes),
    Err(Error),
}

#[cfg(test)]
impl std::cmp::PartialEq for ParseResult {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ParseResult::Done, ParseResult::Done) => true,
            (ParseResult::NotReady, ParseResult::NotReady) => true,
            (ParseResult::Err(_), ParseResult::Err(_)) => false,
            (ParseResult::Ready(ref bs), ParseResult::Ready(ref other_bs)) => bs == other_bs,
            _ => false,
        }
    }
}

pub(crate) enum Parser {
    Boundary(BoundaryParser),
}

impl Parser {
    pub(crate) fn from_with_capacity(
        headers: http::header::HeaderMap<http::header::HeaderValue>,
        capacity: usize,
    ) -> Result<Self, Error> {
        let content_type = headers
            .get(http::header::CONTENT_TYPE)
            .ok_or(Error::ContentTypeMissing)?;

        let mime_type = content_type
            .to_str()
            .map_err(Error::InvalidHeader)
            .and_then(|s| s.parse::<mime::Mime>().map_err(Error::InvalidMimeType))?;

        if mime_type.type_() != mime::MULTIPART {
            return Err(Error::NotMultipart);
        }

        match mime_type.get_param("boundary") {
            Some(boundary) => {
                log::debug!("Creating Boundary Parser");
                let bp = BoundaryParser::with_capacity(boundary, capacity);
                Ok(Parser::Boundary(bp))
            }

            None => return Err(Error::malformed("mime param boundary missing")),
        }
    }

    pub fn parse(&mut self, chunk: hyper::body::Chunk) -> ParseResult {
        match self {
            Parser::Boundary(ref mut inner) => inner.parse(chunk),
        }
    }
}

#[derive(Debug)]
pub(crate) struct BoundaryParser {
    boundary: String,
    buffer: BytesMut,
}

impl BoundaryParser {
    pub fn with_capacity<S: AsRef<str>>(boundary: S, capacity: usize) -> Self {
        let boundary = format!("--{}", boundary.as_ref());

        log::debug!("Creating with boundary: {:?}", boundary);

        Self {
            boundary,
            buffer: BytesMut::with_capacity(capacity),
        }
    }

    pub fn parse<T: AsRef<[u8]>>(&mut self, chunk: T) -> ParseResult {
        // Read the starting boundary.
        self.buffer.extend(chunk.as_ref());
        let boundary = self.boundary.as_bytes();

        if self.buffer.len() < boundary.len() {
            return ParseResult::NotReady;
        }

        // normally there are no preambles, so we could skip this search by just checking if the initial bytes
        // equals our boundary. To `optimize` the common case.

        // Find the start, might have to skip the preamble. It is to be discarded.
        let mut part_start = match twoway::find_bytes(&self.buffer, boundary) {
            None => return ParseResult::NotReady,
            Some(i) => i + boundary.len(),
        };

        const CRLF: &[u8] = &[13, 10]; // "\r\n"
        const BOUNDARY_LAST_PART_SENTINEL: &[u8] = &[45, 45]; // "--"

        // the next two bytes are either CRLF or --.
        match &self.buffer[part_start..part_start + 2] {
            CRLF => {
                // This is not the last part, just skip the linefeed.
                part_start += 2;
            }

            BOUNDARY_LAST_PART_SENTINEL => {
                log::debug!("Found stop sentinel at index: {}", part_start);

                return ParseResult::Done;
            }

            slice => {
                return ParseResult::Err(Error::malformed(format!(
                    "Boundary must be followed by `--` or `\r\n`, found: {:?}",
                    slice
                )));
            }
        }

        match twoway::find_bytes(&self.buffer[part_start..], boundary) {
            Some(i) => {
                // We've found an entire part, snap it of and return it.

                self.buffer.advance(part_start);
                let part_bs = self.buffer.split_to(i - 2).freeze();
                self.buffer.advance(2); // advance past the leading crlf in the next part.
                ParseResult::Ready(part_bs)
            }

            None => ParseResult::NotReady,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn parse_simple_boundary() {
        let data = "\r
\r
--simple boundary\r
\r
Part1\r
--simple boundary\r
Content-type: text/plain; charset=us-ascii\r
\r
Part2\r
\r
--simple boundary--\r
";

        let mut p = BoundaryParser::with_capacity("simple boundary", 500);

        let exp = "\r
Part1";

        assert_eq!(ParseResult::Ready(exp.into()), p.parse(data));

        let exp = "Content-type: text/plain; charset=us-ascii\r
\r
Part2\r
";

        assert_eq!(ParseResult::Ready(exp.into()), p.parse(data));
        assert_eq!(ParseResult::Done, p.parse(data));
    }

    #[test]
    fn parse_boundary_without_leading_crlf() {
        let data = "--simple boundary\r
\r
Part1\r
--simple boundary\r
Content-type: text/plain; charset=us-ascii\r
\r
Part2\r
\r
--simple boundary--\r
";

        let mut p = BoundaryParser::with_capacity("simple boundary", 500);

        let exp = "\r
Part1";

        assert_eq!(ParseResult::Ready(exp.into()), p.parse(data));

        let exp = "Content-type: text/plain; charset=us-ascii\r
\r
Part2\r
";

        assert_eq!(ParseResult::Ready(exp.into()), p.parse(data));
        assert_eq!(ParseResult::Done, p.parse(data));
    }

    #[test]
    fn parse_boundary_with_preamble() {
        let data = "\r
\r
This is the preamble.  It is to be ignored, though it\r
is a handy place for composition agents to include an\r
explanatory note to non-MIME conformant readers.\r
\r
--simple boundary\r
\r
Part1\r
--simple boundary\r
Content-type: text/plain; charset=us-ascii\r
\r
Part2\r
\r
--simple boundary--\r
\r
This is the epilogue.  It is also to be ignored.\r
\r
";

        let mut p = BoundaryParser::with_capacity("simple boundary", 500);

        let exp = "\r
Part1";

        assert_eq!(ParseResult::Ready(exp.into()), p.parse(data));

        let exp = "Content-type: text/plain; charset=us-ascii\r
\r
Part2\r
";

        assert_eq!(ParseResult::Ready(exp.into()), p.parse(data));
        assert_eq!(ParseResult::Done, p.parse(data));
    }

}
