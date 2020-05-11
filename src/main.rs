#[macro_use]
extern crate clap;

mod lehmer64;

use std::cmp;
use std::convert::Infallible;
use std::task::{Context, Poll};
use std::net::ToSocketAddrs;
use std::pin::Pin;

use bytes::Bytes;
use http::StatusCode;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use human_size::{Byte, ParsingError, Size, SpecificSize};
use rand::{Rng, SeedableRng};
use lehmer64::Lehmer64_3 as RandomGenerator;

use tokio::stream::Stream;

const BUF_SIZE: usize = 8192;

// Strip any extension (like .bin), then parse the remaining
// name as size using the "human size" crate. Also allow
// lowercase variants (like 1000mb.bin).
fn size(name: &str) -> Result<u64, ParsingError> {
    let name = name.split(".").next().unwrap();
    let name = name.replace("kb", "kB");
    let name = name.replace("KB", "kB");
    let sz: Size = match name.parse() {
        Ok(sz) => sz,
        Err(_) => {
            name.to_uppercase().parse()?
        },
    };
    let sz: SpecificSize<Byte> = sz.into();
    Ok(sz.value() as u64)
}

// Stream of random data.
struct RandomStream {
    buf:        [u8; BUF_SIZE],
    rng:        Option<RandomGenerator>,
    length:     u64,
    done:       u64,
}

impl RandomStream {
    // create a new RandomStream.
    fn new(length: u64) -> RandomStream {

        RandomStream{
            buf:    [0u8; BUF_SIZE],
            rng:    Some(RandomGenerator::seed_from_u64(0)),
            length: length,
            done:   0,
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
            rng.fill(&mut this.buf[0..4096]);
            rng.fill(&mut this.buf[4096..8192]);
            this.rng = Some(rng);
            this.done += count;
            Poll::Ready(Some(Ok(Bytes::copy_from_slice(&this.buf[0..count as usize]))))
        }
    }
}

// generate file.
fn file(req: Request<Body>) -> Result<Response<Body>, http::Error> {

    // Get the filename (last element of the path)
    let elems = req.uri().path().split('/').collect::<Vec<_>>();
    if elems.len() < 2 || elems[1].len() == 0 || !elems[1].as_bytes()[0].is_ascii_digit() {
        return Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty());
    }

    // parse it as a size.
    let name = elems[1];
    let sz = match size(name) {
        Ok(sz) => sz,
        Err(_) => return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::empty()),
    };

    // response headers.
    Response::builder()
        .header("Content-Type", "application/binary")
        .header("Content-Disposition", format!("attachment; filename={}", name).as_str())
        .header("Content-Length", sz.to_string().as_str())
        .header("Cache-Control", "no-cache, no-store, no-transform, must-revalidate")
        .header("Pragma", "no-cache")
        .status(StatusCode::OK)
        .body(Body::wrap_stream(RandomStream::new(sz)))
}

// generate dirlist.
fn dirlist() -> Result<Response<Body>, http::Error> {
    let index = include_str!("index.html");
    Response::builder()
        .header("Content-Type", "text/html")
        .status(StatusCode::OK)
        .body(Body::from(index))
}

// handler.
async fn handler(req: Request<Body>) -> Result<Response<Body>, http::Error> {
    if req.uri().path() == "/" {
        dirlist()
    } else {
        file(req)
    }
}

#[tokio::main]
async fn main() {
    let matches = clap_app!(speedtest_fileserver_rs =>
        (version: "0.1")
        (@arg LISTEN: -l --listen +takes_value "ip:port to listen on)")
    )
    .get_matches();

    let listen = matches.value_of("LISTEN").unwrap_or("127.0.0.1:3000");
    let mut addrs = listen.to_socket_addrs().expect("cannot parse address");
    let addr = match addrs.next() {
        Some(addr) => addr,
        None => {
            eprintln!("{}: cannot resolve", listen);
            std::process::exit(1)
        },
    };

    let make_svc = make_service_fn(|_conn| async {
                Ok::<_, http::Error>(service_fn(handler))
                        });

    let server = Server::bind(&addr).serve(make_svc);

    println!("Listening on http://{}", addr);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

