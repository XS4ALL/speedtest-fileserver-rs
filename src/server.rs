//! All the actual API handlers.
//!
use std::sync::Arc;

use http::{Response, StatusCode};
use human_size::{Byte, ParsingError, Size, SpecificSize};
use hyper::body::Body;
use tokio::stream::StreamExt;
use tokio::time::{Duration, Instant};
use warp::{filters::BoxedFilter, Filter, Reply};

use crate::Config;
use crate::randomstream::RandomStream;
use crate::template;

// Relative timeout.
const SEND_TIMEOUT: Duration = Duration::from_secs(20);

// 10GiB is the default max size we support.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct FileServer {
    config: Arc<Config>,
}

impl FileServer {
    pub fn new(config: &Config) -> FileServer {
        FileServer {
            config: Arc::new(config.clone()),
        }
    }

    fn index(&self, agent: String, config: &Config) -> http::Result<http::Response<hyper::Body>> {
        let (text, ct, status) = match template::build(config, agent) {
            Ok(index) => (index, "text/html; charset=utf-8", StatusCode::OK),
            Err(e) => (e.to_string(), "text/plain", StatusCode::INTERNAL_SERVER_ERROR),
        };
        Response::builder()
            .header("Content-Type", ct)
            .status(status)
            .body(Body::from(text))
    }

    // Generate a streaming response with random data.
    fn data(&self, filename: String) -> http::Result<http::Response<hyper::Body>> {
        let max_size = self.config.max_file_size.unwrap_or(MAX_FILE_SIZE);

        // parse size.
        let sz = match size(&filename) {
            Ok(sz) if sz > max_size => {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from("too big"))
            }
            Ok(sz) => sz,
            Err(_) => {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from("cannot parse size"))
            }
        };

        // wrap the RandomStream in another stream, so we can handle timeouts etc.
        let stream = Box::pin(async_stream::stream! {
            let mut strm = RandomStream::new(sz);
            let mut timeout = tokio::time::delay_for(SEND_TIMEOUT);

            loop {
                let value = tokio::select! {
                    value = strm.next() => value.unwrap(),
                    _ = &mut timeout => break,
                };
                timeout.reset(Instant::now() + SEND_TIMEOUT);
                yield value;
            }
        });

        // response headers and body.
        Response::builder()
            .header("Content-Type", "application/binary")
            .header(
                "Content-Disposition",
                format!("attachment; filename={}", filename).as_str(),
            )
            .header("Content-Length", sz.to_string().as_str())
            .header(
                "Cache-Control",
                "no-cache, no-store, no-transform, must-revalidate",
            )
            .header("Pragma", "no-cache")
            .status(StatusCode::OK)
            .body(Body::wrap_stream(stream))
    }

    // bundle up "index" and "data" into one Filter.
    pub fn routes(&self) -> BoxedFilter<(impl Reply,)> {
        let config = self.config.clone();
        let this = self.clone();
        let index = warp::path::end()
            .and(warp::header("user-agent"))
            .map(move |agent: String| this.index(agent, &config));

        let this = self.clone();
        let data = warp::path::param()
            .and(warp::path::end())
            .map(move |param: String| this.data(param));

        data.or(index).boxed()
    }
}

// Strip any extension (like .bin), then parse the remaining
// name as size using the "human size" crate. Also allow
// lowercase variants (like 1000mb.bin).
pub fn size(name: &str) -> Result<u64, ParsingError> {
    let name = name.split(".").next().unwrap();
    let name = name.replace("kb", "kB");
    let name = name.replace("KB", "kB");
    let sz: Size = match name.parse() {
        Ok(sz) => sz,
        Err(_) => name.to_uppercase().parse()?,
    };
    let sz: SpecificSize<Byte> = sz.into();
    Ok(sz.value() as u64)
}

