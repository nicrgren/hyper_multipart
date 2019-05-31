use bytes::Bytes;
use http::header::{HeaderMap, HeaderName, HeaderValue};

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
    pub fn header_lines(&self) -> impl Iterator<Item = Result<&str, std::str::Utf8Error>> {
        let slice = &self.headers_data;
        slice.split(|e| *e == b'\n').map(|line| {
            // trim of the last \r
            std::str::from_utf8(line).map(|s| s.trim())
        })
    }

    pub fn headers(&self) -> HeaderMap<HeaderValue> {
        let mut res = HeaderMap::new();

        self.header_lines()
            .filter_map(|line| line.ok())
            .filter_map(|s| parse_header_line(s))
            .for_each(|(name, value)| {
                res.insert(name, value);
            });

        res
    }
}

fn parse_header_line(s: &str) -> Option<(HeaderName, HeaderValue)> {
    if let None = s.find(":") {
        return None;
    }

    let mut parts = s.split(":");

    let header_name = parts
        .next()
        .map(|s| HeaderName::from_bytes(s.trim().as_bytes()));

    let header_value = parts.next().map(|s| HeaderValue::from_str(s.trim()));

    match (header_name, header_value) {
        (Some(Ok(name)), Some(Ok(value))) => Some((name, value)),
        _ => None,
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

impl From<&[u8]> for Part {
    fn from(bs: &[u8]) -> Self {
        // split headers and body

        match twoway::find_bytes(&bs[..], b"\r\n\r\n") {
            // No headers
            None => Part {
                headers_data: Bytes::with_capacity(0),
                body_data: bs.to_vec().into(),
            },
            Some(i) => {
                let header_end = i + 4;

                Part {
                    headers_data: bs[0..i].to_vec().into(),
                    body_data: bs[header_end..bs.len()].to_vec().into(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_header_lines() {
        let tests = [
            ("Content-Type: image/jpeg", "content-type", "image/jpeg"),
            ("Content-Length: 40669", "content-length", "40669"),
            (
                "X-Timestamp: 1550567095.266",
                "x-timestamp",
                "1550567095.266",
            ),
            (
                "X-SendTimestamp: 1550567095.439",
                "x-sendtimestamp",
                "1550567095.439",
            ),
            ("X-TimeDiff: 173", "x-timediff", "173"),
        ];

        for (header, exp_name, exp_val) in &tests {
            let (name, val) = parse_header_line(header).expect("Parse header line");

            assert_eq!(exp_name, &name.as_str());
            assert_eq!(
                exp_val,
                &val.to_str().expect("Converting header value to_str")
            );
        }
    }

}
