use futures::{Future, Stream};
use http::Uri;
use hyper_multipart::{Error, MultipartChunks, MultipartResponse};
use log::{debug, error};
use std::time::Duration;
use tokio::prelude::StreamExt;

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
                    handle_stream(multipart_stream);
                    Ok(())
                }

                Err(e) => Err(Error::from(e)),
            },
        )
        .map_err(|e| error!("Error: {}", e));

    tokio::run(f)
}

pub fn handle_stream(s: MultipartChunks<hyper::Body>) {
    let stream = s
        .throttle(Duration::from_millis(1500))
        .inspect(|part| {
            let headers = part.headers();

            let ts = headers.get("x-timestamp");
            let sent_ts = headers.get("x-sendtimestamp");

            println!("Timestamp: {:?}.     Sent At: {:?}", ts, sent_ts);
        })
        .for_each(|_| Ok(()))
        .map_err(|e| error!("Print stream: {}", e));

    tokio::spawn(stream);
}
