use futures::{Future, Stream};
use http::Uri;
use hyper_multipart::{Error, Multipart, MultipartChunks};
use log::{debug, error};

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
                    print_stream(multipart_stream);
                    Ok(())
                }

                Err(e) => Err(Error::from(e)),
            },
        )
        .map_err(|e| error!("Error: {}", e));

    tokio::run(f)
}

pub fn print_stream(s: MultipartChunks<hyper::Body>) {
    let print_loop = s
        .inspect(|part| {
            debug!("==========================================");
            debug!("New part (body size: {}):", part.body_len());

            for header in part.header_lines() {
                debug!("Header: {:?}", header);
            }
        })
        .for_each(|_| Ok(()))
        .map_err(|e| error!("Print stream: {}", e));

    tokio::spawn(print_loop);
}
