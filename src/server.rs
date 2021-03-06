//!
//! All the actual API handlers.
//!
use std::sync::{Arc, Mutex};

use http::{Response, StatusCode};
use human_size::{Byte, ParsingError, Size, SpecificSize};
use hyper::body::Body;
use tokio_stream::StreamExt;
use tokio::time::{Duration, Instant};
use warp::reply::Response as HyperResponse;
use warp::{filters::BoxedFilter, Filter, Reply};

use crate::logger::LogInfo;
use crate::randomstream::RandomStream;
use crate::template;
use crate::Config;

// Relative timeout.
const SEND_TIMEOUT: Duration = Duration::from_secs(20);

// 10GiB is the default max size we support.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct FileServer {
    config: Arc<Config>,
    access_log: Option<Arc<Mutex<String>>>,
}

impl FileServer {
    pub fn new(config: &Config) -> FileServer {
        let access_log = config.access_log.clone();
        FileServer {
            config: Arc::new(config.clone()),
            access_log: access_log.map(|a| Arc::new(Mutex::new(a))),
        }
    }

    fn index(&self, agent: String, config: &Config) -> http::Result<HyperResponse> {
        let (text, ct, status) = match template::build(config, agent) {
            Ok(index) => (index, "text/html; charset=utf-8", StatusCode::OK),
            Err(e) => (
                e.to_string(),
                "text/plain",
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        };
        Response::builder()
            .header("Content-Type", ct)
            .status(status)
            .body(Body::from(text))
    }

    // Generate a streaming response with random data.
    fn data(&self, filename: String, mut log_info: LogInfo) -> http::Result<HyperResponse> {
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
                let is_num = filename
                    .chars()
                    .next()
                    .map(|c| c.is_numeric())
                    .unwrap_or(false);
                if is_num {
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from("cannot parse size"));
                } else {
                    return Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Body::from("Not Found"));
                };
            }
        };

        // wrap the RandomStream in another stream, so we can handle timeouts etc.
        let stream = Box::pin(async_stream::stream! {
            let mut strm = RandomStream::new(sz);
            let mut timeout = Box::pin(tokio::time::sleep(SEND_TIMEOUT));

            loop {
                let value = tokio::select! {
                    value = strm.next() => {
                        match value {
                            Some(value) => value,
                            None => break,
                        }
                    }
                    _ = timeout.as_mut() => break,
                };
                timeout.as_mut().reset(Instant::now() + SEND_TIMEOUT);
                yield value;
            }
        });

        // response headers and body.
        let resp = Response::builder()
            .header("content-type", "application/binary")
            .header(
                "content-disposition",
                format!("attachment; filename={}", filename).as_str(),
            )
            .header("content-length", sz.to_string().as_str())
            .header(
                "cache-control",
                "no-cache, no-store, no-transform, must-revalidate",
            )
            .header("pragma", "no-cache")
            .header("connection", "close")
            .status(StatusCode::OK);
        log_info.log_on_drop(self.access_log.clone(), self.config.xff);
        log_info.wrap(resp, stream)
    }

    fn log(&self, info: warp::log::Info) {
        // Don't log streams here.
        let file = info.path().split('/').last().unwrap();
        let is_num = file.chars().next().map(|c| c.is_numeric()).unwrap_or(false);
        if is_num && info.status() == http::StatusCode::OK {
            return;
        }

        // Do log everything else.
        let mut log_info =
            LogInfo::from_warp_log_info(info, self.access_log.clone(), self.config.xff);
        log_info.log();
    }

    fn redirect(
        &self,
        uri: Option<&http::Uri>,
    ) -> impl Filter<Extract = (impl Reply,), Error = warp::reject::Rejection> + Clone {
        let uri = uri.cloned();
        warp::any()
            .map(move || uri.clone())
            .and_then(|uri: Option<http::Uri>| async move {
                match uri {
                    Some(uri) => Ok(warp::redirect::temporary(uri)),
                    None => Err(warp::reject::not_found()),
                }
            })
    }

    // bundle up "index" and "data" into one Filter.
    pub fn routes(&self, redirect_uri: Option<&http::Uri>) -> BoxedFilter<(impl Reply,)> {
        let config = self.config.clone();
        let this = self.clone();
        let index = warp::path::end()
            .and(warp::header("user-agent"))
            .map(move |agent: String| this.index(agent, &config));

        let this = self.clone();
        let data = warp::path::param()
            .and(warp::path::end())
            .and(LogInfo::new())
            .map(move |param: String, log_info: LogInfo| this.data(param, log_info));

        let this = self.clone();
        self.redirect(redirect_uri)
            .or(data)
            .or(index)
            .with(warp::log::custom(move |info| this.log(info)))
            .boxed()
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
