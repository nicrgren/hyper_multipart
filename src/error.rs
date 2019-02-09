use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum Error {
    Custom(String),
    Http(hyper::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Custom(ref s) => write!(f, "Error: {}", s),
            Error::Http(ref e) => write!(f, "Http: {}", e),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &'static str {
        match *self {
            Error::Custom(_) => "Any error contained reprsented as a String",
            Error::Http(_) => "Http error thrown by Hyper",
        }
    }

    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match *self {
            Error::Custom(_) => None,
            Error::Http(ref e) => e.source(),
        }
    }
}

impl From<hyper::Error> for Error {
    fn from(inner: hyper::Error) -> Self {
        Error::Http(inner)
    }
}
