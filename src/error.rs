use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum Error {
    /// Cannot turn a non multipart response into multipart.
    ContentTypeMissing,
    NotMultipart,
    MalformedMultipart(String),
    InvalidMimeType(mime::FromStrError),
    InnerStream(String),
}

impl Error {
    pub(crate) fn malformed<S: Into<String>>(msg: S) -> Self {
        Error::MalformedMultipart(msg.into())
    }

    pub(crate) fn inner<E: fmt::Display + Send + 'static>(e: E) -> Self {
        Error::InnerStream(format!("{}", e))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::ContentTypeMissing => write!(f, "Content Type header missing from response"),
            Error::MalformedMultipart(ref msg) => write!(f, "Malformed Multipart: {}", msg),
            Error::NotMultipart => {
                write!(f, "Cannot handle a non multipart response as multipart.")
            }
            Error::InvalidMimeType(ref e) => write!(f, "Content-Type value invalid: {}", e),
            Error::InnerStream(ref e) => write!(f, "InnerStream: {}", e),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &'static str {
        match *self {
            Error::ContentTypeMissing => "Content type header was missing from http response",
            Error::MalformedMultipart(_) => "Ran into errors when parsing multipart",
            Error::NotMultipart => "The Http response was not a multipart",
            Error::InvalidMimeType(_) => {
                "Value of the Content Type header contained an invalid mime type"
            }
            Error::InnerStream(_) => "Http error thrown by the underlying layer",
        }
    }

    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match *self {
            Error::MalformedMultipart(_) => None,
            Error::InvalidMimeType(ref e) => Some(e),
            Error::InnerStream(_) => None,
            _ => None,
        }
    }
}

impl From<hyper::Error> for Error {
    fn from(inner: hyper::Error) -> Self {
        Error::InnerStream(format!("Hyper error: {}", inner))
    }
}
