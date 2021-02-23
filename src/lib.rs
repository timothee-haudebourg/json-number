use std::{
	ops::Deref,
	convert::TryFrom
};

pub struct InvalidNumber;

pub struct Number {
	data: [u8]
}

impl Number {
	pub fn parse<B: AsRef<[u8]> + ?Sized>(data: &B) -> Result<&Number, InvalidNumber> {
		let data = data.as_ref();

		enum State {
			Init,
			FirstDigit,
			Zero,
			NonZero,
			FractionalFirst,
			FractionalRest,
			ExponentSign,
			ExponentFirst,
			ExponentRest
		}

		let mut state = State::Init;
		let mut len = 0usize;
		let mut valid_len = 0usize;

		for b in data {
			match state {
				State::Init => match *b {
					b'-' => state = State::FirstDigit,
					b'0' => state = State::Zero,
					b'1' ..= b'9' => state = State::NonZero,
					_ => break
				},
				State::FirstDigit => match *b {
					b'0' => state = State::Zero,
					b'1' ..= b'9' => state = State::NonZero,
					_ => break
				},
				State::Zero => match *b {
					b'.' => state = State::FractionalFirst,
					b'e' | b'E' => state = State::ExponentSign,
					_ => break
				},
				State::NonZero => match *b {
					b'0' ..= b'9' => state = State::NonZero,
					b'.' => state = State::FractionalFirst,
					b'e' | b'E' => state = State::ExponentSign,
					_ => break
				},
				State::FractionalFirst => match *b {
					b'0' ..= b'9' => state = State::FractionalRest,
					_ => break
				},
				State::FractionalRest => match *b {
					b'0' ..= b'9' => state = State::FractionalRest,
					b'e' | b'E' => state = State::ExponentSign,
					_ => break
				},
				State::ExponentSign => match *b {
					b'+' | b'-' => state = State::ExponentFirst,
					b'0' ..= b'9' => state = State::ExponentRest,
					_ => break
				},
				State::ExponentFirst => match *b {
					b'0' ..= b'9' => state = State::ExponentRest,
					_ => break
				},
				State::ExponentRest => match *b {
					b'0' ..= b'9' => state = State::ExponentRest,
					_ => break
				}
			}

			len += 1;

			match state {
				State::Zero | State::NonZero | State::FractionalRest | State::ExponentRest => valid_len = len,
				_ => ()
			}
		}

		if valid_len > 0 {
			Ok(unsafe {
				// safe because we just parsed the data.
				Self::new_unchecked(&data[0..len])
			})
		} else {
			Err(InvalidNumber)
		}
	}

	#[inline]
	pub fn new<B: AsRef<[u8]> + ?Sized>(data: &B) -> Result<&Number, InvalidNumber> {
		let data = data.as_ref();
		let n = Self::parse(data)?;
		if n.len() == data.len() {
			Ok(n)
		} else {
			Err(InvalidNumber)
		}
	}

	#[inline]
	pub unsafe fn new_unchecked<B: AsRef<[u8]> + ?Sized>(data: &B) -> &Number {
		std::mem::transmute(data.as_ref())
	}

	#[inline]
	pub fn len(&self) -> usize {
		self.data.len()
	}

	#[inline]
	pub fn as_str(&self) -> &str {
		unsafe {
			// safe because `self.data` is always a valid UTF-8 sequence.
			std::str::from_utf8_unchecked(&self.data)
		}
	}

	#[inline]
	pub fn is_i32(&self) -> bool {
		self.as_i32().is_some()
	}

	#[inline]
	pub fn is_i64(&self) -> bool {
		self.as_i64().is_some()
	}

	#[inline]
	pub fn is_u32(&self) -> bool {
		self.as_u32().is_some()
	}

	#[inline]
	pub fn is_u64(&self) -> bool {
		self.as_u64().is_some()
	}

	#[inline]
	pub fn is_f32(&self) -> bool {
		self.as_f32().is_some()
	}

	#[inline]
	pub fn is_f64(&self) -> bool {
		self.as_f64().is_some()
	}

	#[inline]
	pub fn as_i32(&self) -> Option<i32> {
		self.as_str().parse().ok()
	}

	#[inline]
	pub fn as_i64(&self) -> Option<i64> {
		self.as_str().parse().ok()
	}

	#[inline]
	pub fn as_u32(&self) -> Option<u32> {
		self.as_str().parse().ok()
	}

	#[inline]
	pub fn as_u64(&self) -> Option<u64> {
		self.as_str().parse().ok()
	}

	#[inline]
	pub fn as_f32(&self) -> Option<f32> {
		self.as_str().parse::<f32>().ok().filter(|f| f.is_finite())
	}

	#[inline]
	pub fn as_f64(&self) -> Option<f64> {
		self.as_str().parse::<f64>().ok().filter(|f| f.is_finite())
	}
}

impl<'a> TryFrom<&'a str> for &'a Number {
	type Error = InvalidNumber;

	#[inline]
	fn try_from(s: &'a str) -> Result<&'a Number, InvalidNumber> {
		Number::new(s)
	}
}

pub struct NumberBuf {
	data: Vec<u8>
}

impl NumberBuf {
	pub fn new<B: Into<Vec<u8>>>(data: B) -> Result<NumberBuf, InvalidNumber> {
		let data = data.into();
		Number::new(&data)?;
		Ok(NumberBuf { data })
	}

	pub unsafe fn new_unchecked<B: Into<Vec<u8>>>(data: B) -> NumberBuf {
		let data = data.into();
		NumberBuf { data }
	}
}

impl Deref for NumberBuf {
	type Target = Number;

	fn deref(&self) -> &Number {
		unsafe {
			Number::new_unchecked(&self.data)
		}
	}
}

macro_rules! impl_from_int {
	($($ty:ty),*) => {
		$(
			impl From<$ty> for NumberBuf {
				#[inline]
				fn from(i: $ty) -> Self {
					unsafe {
						Self::new_unchecked(itoa::Buffer::new().format(i).to_owned())
					}
				}
			}
		)*
	};
}

macro_rules! impl_from_float {
	($($ty:ty),*) => {
		$(
			impl From<$ty> for NumberBuf {
				#[inline]
				fn from(f: $ty) -> Self {
					unsafe {
						Self::new_unchecked(ryu::Buffer::new().format_finite(f).to_owned())
					}
				}
			}
		)*
	};
}

impl_from_int!(u8, i8, u16, i16, u32, i32, u64, i64, usize, isize);
impl_from_float!(f32, f64);