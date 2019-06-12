/// A header source, implemented for http::HeaderMap.
/// If library is to be used for actix, it must be implemented
/// for actix_web::HeaderMap
pub trait HeaderMap {
    fn get_value<K>(&self, header_key: K) -> Option<&str>
    where
        K: AsRef<str>;
}

impl HeaderMap for http::header::HeaderMap {
    fn get_value<K>(&self, header_key: K) -> Option<&str>
    where
        K: AsRef<str>,
    {
        self.get(header_key.as_ref())
            .and_then(|hv| hv.to_str().ok())
    }
}
