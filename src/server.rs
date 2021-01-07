//! All the actual API handlers.
//!
use std::cmp;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use http::{Response, StatusCode};
use human_size::{Byte, ParsingError, Size, SpecificSize};
use hyper::body::Body;
use rand::{Rng, SeedableRng};
use tokio::stream::{Stream, StreamExt};
use tokio::time::{Duration, Instant};
use warp::{filters::BoxedFilter, Filter, Reply};

use crate::lehmer64::Lehmer64_3 as RandomGenerator;
use crate::Config;

// Relative timeout.
const SEND_TIMEOUT: Duration = Duration::from_secs(20);

// 10GB is the max size we support, set actual max a bit higher.
const MAX_SIZE: u64 = 11_000_000_000;

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

    fn index(&self) -> http::Result<http::Response<hyper::Body>> {
        let index = include_str!("index.html");
        Response::builder()
            .header("Content-Type", "text/html")
            .status(StatusCode::OK)
            .body(Body::from(index))
    }

    // Generate a streaming response with random data.
    fn data(&self, filename: String) -> http::Result<http::Response<hyper::Body>> {
        // parse size.
        let sz = match size(&filename) {
            Ok(sz) if sz > MAX_SIZE => {
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
        let this = self.clone();
        let index = warp::path::end().map(move || this.index());

        let this = self.clone();
        let data = warp::path::param()
            .and(warp::path::end())
            .map(move |param: String| this.data(param));

        data.or(index).boxed()
    }
}

const CHUNK_SIZE: usize = 4096;
const NUM_CHUNKS: usize = 4;
const BUF_SIZE: usize = CHUNK_SIZE * NUM_CHUNKS;

// Strip any extension (like .bin), then parse the remaining
// name as size using the "human size" crate. Also allow
// lowercase variants (like 1000mb.bin).
fn size(name: &str) -> Result<u64, ParsingError> {
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

// Stream of random data.
struct RandomStream {
    buf: [u8; BUF_SIZE],
    rng: Option<RandomGenerator>,
    length: u64,
    done: u64,
}

impl RandomStream {
    // create a new RandomStream.
    fn new(length: u64) -> RandomStream {
        RandomStream {
            buf: [0u8; BUF_SIZE],
            rng: Some(RandomGenerator::seed_from_u64(0)),
            length: length,
            done: 0,
        }
    }
}

impl Stream for RandomStream {
    type Item = Result<Bytes, Infallible>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Option<Self::Item>> {
        let this = self;
        tokio::pin!(this);
        if this.done >= this.length {
            Poll::Ready(None)
        } else {
            // generate block of random data.
            let count = cmp::min(this.length - this.done, BUF_SIZE as u64);
            let mut rng = this.rng.take().unwrap();
            for i in 0..NUM_CHUNKS {
                let start = i * CHUNK_SIZE;
                let end = (i + 1) * CHUNK_SIZE;
                rng.fill(&mut this.buf[start..end]);
            }
            this.rng = Some(rng);
            this.done += count;
            Poll::Ready(Some(Ok(Bytes::copy_from_slice(
                &this.buf[0..count as usize],
            ))))
        }
    }
}
