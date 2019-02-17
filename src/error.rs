use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum Error {
    /// Cannot turn a non multipart response into multipart.
    ContentTypeMissing,
    NotMultipart,
    InvalidHeader(http::header::ToStrError),
    InvalidMimeType(mime::FromStrError),
    Http(hyper::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::ContentTypeMissing => write!(f, "Content Type header missing from response"),
            Error::NotMultipart => {
                write!(f, "Cannot handle a non multipart response as multipart.")
            }
            Error::InvalidHeader(ref e) => write!(f, "Could not parse Content Type header: {}", e),
            Error::InvalidMimeType(ref e) => write!(f, "Content-Type value invalid: {}", e),
            Error::Http(ref e) => write!(f, "Http: {}", e),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &'static str {
        match *self {
            Error::ContentTypeMissing => "Content type header was missing from http response",
            Error::NotMultipart => "The Http response was not a multipart",
            Error::InvalidHeader(_) => "Value of the Content Type header could not be parsed",
            Error::InvalidMimeType(_) => {
                "Value of the Content Type header contained an invalid mime type"
            }
            Error::Http(_) => "Http error thrown by Hyper",
        }
    }

    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match *self {
            Error::InvalidHeader(ref e) => Some(e),
            Error::InvalidMimeType(ref e) => Some(e),
            Error::Http(ref e) => e.source(),
            _ => None,
        }
    }
}

impl From<hyper::Error> for Error {
    fn from(inner: hyper::Error) -> Self {
        Error::Http(inner)
    }
}
