use futures::{Future, Stream};
use http::Uri;
use hyper_multipart::{Error, MultipartResponse, Part};
use log::{debug, error};
use std::time;
use tokio::fs::file::File;

fn main() {
    dotenv::dotenv().expect("Failed to initialize dotenv");
    pretty_env_logger::init();

    let stream_url = std::env::var("STREAM_URL").expect("STREAM_URL must be set");

    let client = hyper::Client::new();
    let target_uri: Uri = stream_url.parse().expect("Invalid stream URL");

    let f = client
        .get(target_uri)
        .map_err(Error::from)
        .and_then(
            |response: hyper::Response<hyper::Body>| match response.into_multipart() {
                Ok(multipart_stream) => {
                    store_n_images(10, multipart_stream);
                    Ok(())
                }

                Err(e) => Err(Error::from(e)),
            },
        )
        .map_err(|e| debug!("Error: {}", e));

    tokio::run(f)
}

pub fn store_n_images(n: u64, s: impl Stream<Item = Part, Error = Error> + Send + 'static) {
    let f = s
        .take(n)
        .for_each(|part| {
            debug!("Storing a with bodysize {}", part.body_len());
            let filename = format!("camsnap-{}.jpg", now());

            let write_fut = File::create(filename)
                .and_then(|file| tokio::io::write_all(file, part.into_body()))
                .map(|_| {
                    debug!("Wrote stuffs");
                })
                .map_err(|e| error!("Failed to write file: {}", e));

            tokio::spawn(write_fut);

            Ok(())
        })
        .map_err(|e| debug!("Error during store images: {}", e));

    tokio::spawn(f);
}

fn now() -> u64 {
    let now = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .unwrap();
    let ms = now.as_secs() * 1000;
    ms + now.subsec_millis() as u64
}
