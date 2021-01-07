use std::convert::TryInto;
use rand_core::{impls, Error, RngCore, SeedableRng};

#[derive(Default)]
pub struct Lehmer64_3 {
	state:		[u128; 3],
	pos:		u32,
}

#[inline]
fn mul(a: &mut u128, b: u128) {
    *a = u128::overflowing_mul(*a, b).0;
}

impl Lehmer64_3 {
    #[inline]
	fn next(&mut self) -> u64 {
		self.pos += 1;
		if self.pos == 3 {
            mul(&mut self.state[0], 0xda942042e4dd58b5u128);
            mul(&mut self.state[1], 0xda942042e4dd58b5u128);
            mul(&mut self.state[2], 0xda942042e4dd58b5u128);
			self.pos = 0;
		}
		(self.state[self.pos as usize] >> 64) as u64
	}
}

impl RngCore for Lehmer64_3 {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.next() as u32
    }
    #[inline]
    fn next_u64(&mut self) -> u64 {
		self.next()
    }
    #[inline]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        impls::fill_bytes_via_next(self, dest)
    }
    #[inline]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        impls::fill_bytes_via_next(self, dest);
        Ok(())
    }
}

impl SeedableRng for Lehmer64_3 {
    type Seed = [u8; 24];

    fn from_seed(seed: Self::Seed) -> Self {
		let n1 = u64::from_be_bytes(seed[0..8].try_into().unwrap());
		let n2 = u64::from_be_bytes(seed[8..16].try_into().unwrap());
		let n3 = u64::from_be_bytes(seed[16..24].try_into().unwrap());
		Lehmer64_3 {
			state: [ n1 as u128, n2 as u128, n3 as u128 ],
			pos: 2,
		}
    }
}

#[derive(Default)]
pub struct Lehmer64(u128);

impl Lehmer64 {
    #[inline]
	fn next(&mut self) -> u64 {
        self.0 *= 0xda942042e4dd58b5u128;
		(self.0 >> 64) as u64
	}
}

impl RngCore for Lehmer64 {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.next() as u32
    }
    #[inline]
    fn next_u64(&mut self) -> u64 {
		self.next()
    }
    #[inline]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        impls::fill_bytes_via_next(self, dest)
    }
    #[inline]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        impls::fill_bytes_via_next(self, dest);
        Ok(())
    }
}

impl SeedableRng for Lehmer64 {
    type Seed = [u8; 8];

    fn from_seed(seed: Self::Seed) -> Self {
		let n = u64::from_be_bytes(seed.try_into().unwrap());
		Lehmer64(n as u128)
    }
}

