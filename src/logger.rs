use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;

use hyper::body::Body;
use tokio::stream::Stream;
use warp::reply::Response as HyperResponse;
use warp::Filter;

use crate::remoteip;

/// A LogInfo keeps the same kind of info as a warp::log::Info, but it
/// also keeps a byte counter, and can log-on-drop, so it is possible
/// to log the amount of bytes transfered for a streaming body.
pub struct LogInfo {
    data: Option<LogInfoData>,
    access_log: Option<Arc<Mutex<String>>>,
    do_xff: bool,
}

#[derive(Clone)]
struct LogInfoData {
    start: Instant,
    remote_addr: Option<SocketAddr>,
    method: http::Method,
    status: http::StatusCode,
    path: String,
    version: http::Version,
    length: u64,
    referer: Option<String>,
    agent: Option<String>,
    xff: Option<String>,
    xri: Option<String>,
    fwd: Option<String>,
}

impl LogInfo {
    pub fn new() -> impl Filter<Extract = (LogInfo,), Error = warp::reject::Rejection> + Copy {
        warp::addr::remote()
            .and(warp::method())
            .and(warp::path::full())
            .and(warp::header::optional::<String>("referer"))
            .and(warp::header::optional::<String>("user-agent"))
            .and(warp::header::optional::<String>("x-forwarded-for"))
            .and(warp::header::optional::<String>("x-real-ip"))
            .and(warp::header::optional::<String>("forwarded"))
            .map(
                |addr: Option<SocketAddr>,
                 method: http::Method,
                 path: warp::path::FullPath,
                 referer: Option<String>,
                 agent: Option<String>,
                 xff: Option<String>,
                 xri: Option<String>,
                 fwd: Option<String>| {
                    let data = LogInfoData {
                        start: Instant::now(),
                        remote_addr: addr,
                        method,
                        status: http::StatusCode::OK,
                        path: path.as_str().to_string(),
                        version: http::Version::HTTP_11,
                        length: 0,
                        referer,
                        agent,
                        xff,
                        xri,
                        fwd,
                    };
                    LogInfo {
                        data: Some(data),
                        access_log: None,
                        do_xff: false,
                    }
                },
            )
    }

    // Turn a warp::log::Info into a LogInfo.
    #[allow(dead_code)]
    pub fn from_warp_log_info(
        info: warp::log::Info,
        access_log: Option<Arc<Mutex<String>>>,
        do_xff: bool,
    ) -> LogInfo {
        let headers = info.request_headers();
        let data = LogInfoData {
            start: Instant::now(),
            remote_addr: info.remote_addr(),
            method: info.method().clone(),
            status: info.status(),
            path: info.path().to_string(),
            version: info.version(),
            length: 0,
            referer: info.referer().map(|s| s.to_string()),
            agent: info.user_agent().map(|s| s.to_string()),
            xff: headers
                .get("x-forwarded-for")
                .map(|v| v.to_str().ok())
                .flatten()
                .map(|s| s.to_string()),
            xri: headers
                .get("x-real-ip")
                .map(|v| v.to_str().ok())
                .flatten()
                .map(|s| s.to_string()),
            fwd: headers
                .get("forwarded")
                .map(|v| v.to_str().ok())
                .flatten()
                .map(|s| s.to_string()),
        };
        LogInfo {
            access_log,
            do_xff,
            data: Some(data),
        }
    }

    /// Log configuration. Call this before wrapping the response.
    pub fn log_on_drop(&mut self, access_log: Option<Arc<Mutex<String>>>, do_xff: bool) {
        self.access_log = access_log;
        self.do_xff = do_xff;
    }

    /// Wrap the response so we can count the number of bytes transferred and then log.
    pub fn wrap<T, E>(
        self,
        builder: http::response::Builder,
        strm: T,
    ) -> http::Result<HyperResponse>
    where
        T: Stream<Item = Result<bytes::Bytes, E>> + Send + Unpin + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        if self.access_log.is_none() {
            return builder.body(Body::wrap_stream(strm));
        }

        let w = LogCounter {
            strm,
            log_info: self,
        };
        builder.body(Body::wrap_stream(w))
    }

    /// Write a line the access logfile.
    pub fn log(&mut self) {
        // take out access_log and data, so we log only once.
        let (access_log, data) = match (self.access_log.take(), self.data.take()) {
            (Some(a), Some(d)) => (a, d),
            _ => return,
        };

        // open logfile.
        let access_log = access_log.lock().unwrap();
        let mut options = fs::OpenOptions::new();
        let mut file = match options.create(true).append(true).open(access_log.as_str()) {
            Ok(file) => file,
            Err(_) => return,
        };

        // calculate client address.
        let addr = remoteip::parse(
            data.remote_addr,
            self.do_xff,
            data.xff.as_ref(),
            data.xri.as_ref(),
            data.fwd.as_ref(),
        );
        let addr = addr
            .map(|a| a.ip().to_string())
            .unwrap_or(String::from("unknown"));

        let start_date = "";
        let referer = data.referer.as_ref().map(|s| s.as_str()).unwrap_or("");
        let agent = data.agent.as_ref().map(|s| s.as_str()).unwrap_or("");
        let length = if data.length == 0 {
            String::from("-")
        } else {
            data.length.to_string()
        };

        // apache default log format:
        // remote - - [date] "METHOD path version" status length "referer" "agent"
        let _ = writeln!(
            file,
            "{remote} - - {date} \"{method} {path} {version:?}\" {status} {length} \"{referer}\" \"{agent}\"",
            remote = addr,
            date = start_date,
            method = data.method,
            path = data.path,
            version = data.version,
            status = data.status.as_u16(),
            length = length,
            referer = referer,
            agent = agent,
        );
    }
}

struct LogCounter<T> {
    strm: T,
    log_info: LogInfo,
}

impl<T, E> Stream for LogCounter<T>
where
    T: Stream<Item = Result<bytes::Bytes, E>> + Unpin,
{
    type Item = Result<bytes::Bytes, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let strm = Pin::new(&mut self.strm);
        match strm.poll_next(cx) {
            Poll::Ready(Some(Ok(item))) => {
                if let Some(data) = self.log_info.data.as_mut() {
                    data.length += item.len() as u64;
                }
                Poll::Ready(Some(Ok(item)))
            }
            other => other,
        }
    }
}

impl<T> Drop for LogCounter<T> {
    fn drop(&mut self) {
        self.log_info.log()
    }
}
