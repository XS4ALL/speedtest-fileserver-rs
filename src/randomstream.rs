use std::cmp;
use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use rand::{Rng, SeedableRng};
use tokio::stream::Stream;

use crate::lehmer64::Lehmer64_3 as RandomGenerator;

const CHUNK_SIZE: usize = 4096;
const NUM_CHUNKS: usize = 4;
const BUF_SIZE: usize = CHUNK_SIZE * NUM_CHUNKS;

// Stream of random data.
pub struct RandomStream {
    buf: [u8; BUF_SIZE],
    rng: Option<RandomGenerator>,
    length: u64,
    done: u64,
}

impl RandomStream {
    // create a new RandomStream.
    pub fn new(length: u64) -> RandomStream {
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
