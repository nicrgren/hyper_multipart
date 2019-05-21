use chrono::NaiveDateTime;
use futures::{Future, Stream};
use http::Uri;
use hyper_multipart::{Error, Multipart, MultipartChunks};
use log::error;
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
        .throttle(Duration::from_millis(1000))
        .inspect(|part| {
            let headers = part.headers();

            let ts = headers
                .get("x-timestamp")
                .expect("Getting x-timestamp")
                .to_str()
                .expect("Convering x-timestamp to str")
                .parse::<f64>()
                .expect("Parse x-timestamp as f64") as i64;

            let sent_ts: i64 = headers
                .get("x-sendtimestamp")
                .expect("Getting x-sendtimestamp")
                .to_str()
                .expect("Convering x-sendtimestamp to str")
                .parse::<f64>()
                .expect("Parse x-sendtimestamp as f64") as i64;

            let ts_date = NaiveDateTime::from_timestamp(ts, 0);
            let sent_ts_date = NaiveDateTime::from_timestamp(sent_ts, 0);

            println!("Timestamp: {}.     Sent At: {}", ts_date, sent_ts_date);
        })
        .for_each(|_| Ok(()))
        .map_err(|e| error!("Print stream: {}", e));

    tokio::spawn(stream);
}
