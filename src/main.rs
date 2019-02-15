use std::cmp;

use bytes::Bytes;
use futures::prelude::*;
use http;
use hyper::service::service_fn;
use hyper::{self, Body, Request, Response, Server, StatusCode};

use human_size::{Byte, ParsingError, Size, SpecificSize};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoroshiro128StarStar;

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
    buf:        [u8; 4096],
    rng:        Xoroshiro128StarStar,
    length:     u64,
    done:       u64,
}

impl RandomStream {
    // create a new RandomStream.
    fn new(length: u64) -> RandomStream {

        RandomStream{
            buf:    [0u8; 4096],
            rng:    Xoroshiro128StarStar::seed_from_u64(0),
            length: length,
            done:   0,
        }
    }
}

impl Stream for RandomStream {
    type Item = Bytes;
    type Error = http::Error;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        if self.done >= self.length {
            Ok(Async::Ready(None))
        } else {
            // generate block of random data.
            let count = cmp::min(self.length - self.done, self.buf.len() as u64);
            self.rng.fill(&mut self.buf);
            self.done += count;
            Ok(Async::Ready(Some((&self.buf[0..count as usize]).into())))
        }
    }
}

// generate file.
fn file(req: Request<Body>) -> http::Result<Response<Body>> {

    // Get the filename (last element of the path)
    let elems = req.uri().path().split('/').collect::<Vec<_>>();
    if elems.len() < 2 || elems[1].len() == 0 {
        return Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty());
    }

    // parse it as a size.
    let name = elems[1];
    let sz = match size(name) {
        Ok(sz) => sz,
        Err(_) => return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::empty()),
    };

    // response headers.
    let mut response = Response::builder();
    response.header("Content-Type", "application/binary");
    response.header("Content-Disposition", format!("attachment; filename={}", name).as_str());
    response.header("Content-Length", sz.to_string().as_str());
    response.header("Cache-Control", "no-cache, no-store, no-transform, must-revalidate");
    response.header("Pragma", "no-cache");
    response.status(StatusCode::OK);

    // return a stream of random data.
    response.body(Body::wrap_stream(RandomStream::new(sz)))
}

// generate dirlist.
fn dirlist() -> http::Result<Response<Body>> {
    let mut response = Response::builder();
    response.header("Content-Type", "text/html");
    response.status(StatusCode::OK);

    let index = include_str!("index.html");
    response.body(Body::from(index))
}

// handler.
fn handler(req: Request<Body>) -> http::Result<Response<Body>> {
    if req.uri().path() == "/" {
        dirlist()
    } else {
        file(req)
    }
}

fn main() {
    let addr = ([127, 0, 0, 1], 3000).into();

    let server = Server::bind(&addr)
        .serve(|| service_fn(handler))
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);
    hyper::rt::run(server);
}

