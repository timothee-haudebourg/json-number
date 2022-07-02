//! This is a simple library for parsing and storing JSON numbers according
//! to the [JSON specification](https://www.json.org/json-en.html).
//! It provides two types, the unsized `Number` type acting like `str`,
//! and the `NumberBuf<B>` type owning the data inside the `B` type
//! (by default `String`).
//! By enabling the `smallnumberbuf` feature, the `SmallNumberBuf<LEN>` type is
//! defined as `NumberBuf<SmallVec<[u8; LEN]>>` (where `LEN=8` by default).
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

#[cfg(feature = "smallnumberbuf")]
mod smallnumberbuf {
	use super::*;
	use smallvec::SmallVec;

	/// JSON number buffer based on [`SmallVec`](smallvec::SmallVec).
	pub type SmallNumberBuf<const LEN: usize = 8> = NumberBuf<SmallVec<[u8; LEN]>>;
}

#[cfg(feature = "smallnumberbuf")]
pub use smallnumberbuf::*;

#[derive(Clone, Copy, Debug)]
pub struct InvalidNumber<T>(T);

/// Number sign.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Sign {
	Negative,
	Zero,
	Positive,
}

impl Sign {
	/// Checks if the number is zero.
	#[inline]
	pub fn is_zero(&self) -> bool {
		matches!(self, Self::Zero)
	}

	/// Checks if the number is non positive (negative or zero).
	#[inline]
	pub fn is_non_positive(&self) -> bool {
		matches!(self, Self::Negative | Self::Zero)
	}

	/// Checks if the number is non negative (positive or zero).
	#[inline]
	pub fn is_non_negative(&self) -> bool {
		matches!(self, Self::Positive | Self::Zero)
	}

	/// Checks if the number is strictly positive (non zero nor negative).
	#[inline]
	pub fn is_positive(&self) -> bool {
		matches!(self, Self::Positive)
	}

	/// Checks if the number is strictly negative (non zero nor positive).
	#[inline]
	pub fn is_negative(&self) -> bool {
		matches!(self, Self::Negative)
	}
}

/// Lexical JSON number.
///
/// This hold the lexical representation of a JSON number.
/// All the comparison operations are done on this *lexical* representation,
/// meaning that `1` is actually greater than `0.1e+80` for instance.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Number {
	data: [u8],
}

impl Number {
	/// Creates a new number by parsing the given input `data`.
	pub fn new<B: AsRef<[u8]> + ?Sized>(data: &B) -> Result<&Number, InvalidNumber<&B>> {
		let s = data.as_ref();

		enum State {
			Init,
			FirstDigit,
			Zero,
			NonZero,
			FractionalFirst,
			FractionalRest,
			ExponentSign,
			ExponentFirst,
			ExponentRest,
		}

		let mut state = State::Init;

		for b in s {
			match state {
				State::Init => match *b {
					b'-' => state = State::FirstDigit,
					b'0' => state = State::Zero,
					b'1'..=b'9' => state = State::NonZero,
					_ => return Err(InvalidNumber(data)),
				},
				State::FirstDigit => match *b {
					b'0' => state = State::Zero,
					b'1'..=b'9' => state = State::NonZero,
					_ => return Err(InvalidNumber(data)),
				},
				State::Zero => match *b {
					b'.' => state = State::FractionalFirst,
					b'e' | b'E' => state = State::ExponentSign,
					_ => return Err(InvalidNumber(data)),
				},
				State::NonZero => match *b {
					b'0'..=b'9' => state = State::NonZero,
					b'.' => state = State::FractionalFirst,
					b'e' | b'E' => state = State::ExponentSign,
					_ => return Err(InvalidNumber(data)),
				},
				State::FractionalFirst => match *b {
					b'0'..=b'9' => state = State::FractionalRest,
					_ => return Err(InvalidNumber(data)),
				},
				State::FractionalRest => match *b {
					b'0'..=b'9' => state = State::FractionalRest,
					b'e' | b'E' => state = State::ExponentSign,
					_ => return Err(InvalidNumber(data)),
				},
				State::ExponentSign => match *b {
					b'+' | b'-' => state = State::ExponentFirst,
					b'0'..=b'9' => state = State::ExponentRest,
					_ => return Err(InvalidNumber(data)),
				},
				State::ExponentFirst => match *b {
					b'0'..=b'9' => state = State::ExponentRest,
					_ => return Err(InvalidNumber(data)),
				},
				State::ExponentRest => match *b {
					b'0'..=b'9' => state = State::ExponentRest,
					_ => return Err(InvalidNumber(data)),
				},
			}
		}

		if matches!(
			state,
			State::Zero | State::NonZero | State::FractionalRest | State::ExponentRest
		) {
			Ok(unsafe { Self::new_unchecked(s) })
		} else {
			Err(InvalidNumber(data))
		}
	}

	/// Creates a new number without parsing the given input `data`.
	///
	/// ## Safety
	///
	/// The `data` input **must** be a valid JSON number.
	#[inline]
	pub unsafe fn new_unchecked<B: AsRef<[u8]> + ?Sized>(data: &B) -> &Number {
		std::mem::transmute(data.as_ref())
	}

	#[inline]
	pub fn as_str(&self) -> &str {
		unsafe {
			// safe because `self.data` is always a valid UTF-8 sequence.
			std::str::from_utf8_unchecked(&self.data)
		}
	}

	/// Checks if the number is equal to zero (`0`).
	///
	/// This include every lexical representation where
	/// the decimal and fraction part are composed of only
	/// `0`, maybe preceded with `-`, and an arbitrary exponent part.
	#[inline]
	pub fn is_zero(&self) -> bool {
		for b in &self.data {
			match b {
				b'-' | b'0' | b'.' => (),
				b'e' | b'E' => break,
				_ => return false,
			}
		}

		true
	}

	/// Returns the sign of the number.
	pub fn sign(&self) -> Sign {
		let mut non_negative = true;

		for b in &self.data {
			match b {
				b'-' => non_negative = false,
				b'0' | b'.' => (),
				b'e' | b'E' => break,
				_ => {
					return if non_negative {
						Sign::Positive
					} else {
						Sign::Negative
					}
				}
			}
		}

		Sign::Zero
	}

	/// Checks if the number is non positive (negative or zero).
	#[inline]
	pub fn is_non_positive(&self) -> bool {
		self.sign().is_non_positive()
	}

	/// Checks if the number is non negative (positive or zero).
	#[inline]
	pub fn is_non_negative(&self) -> bool {
		self.sign().is_non_negative()
	}

	/// Checks if the number is strictly positive (non zero nor negative).
	#[inline]
	pub fn is_positive(&self) -> bool {
		self.sign().is_positive()
	}

	/// Checks if the number is strictly negative (non zero nor positive).
	#[inline]
	pub fn is_negative(&self) -> bool {
		self.sign().is_negative()
	}

	/// Checks if the number has a fraction part.
	#[inline]
	pub fn has_fraction(&self) -> bool {
		self.data.contains(&b'.')
	}

	/// Checks if the number has an exponent part.
	#[inline]
	pub fn has_exponent(&self) -> bool {
		for b in &self.data {
			if matches!(b, b'e' | b'E') {
				return true;
			}
		}

		false
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

impl Deref for Number {
	type Target = str;

	#[inline]
	fn deref(&self) -> &str {
		self.as_str()
	}
}

impl AsRef<str> for Number {
	#[inline]
	fn as_ref(&self) -> &str {
		self.as_str()
	}
}

impl AsRef<[u8]> for Number {
	#[inline]
	fn as_ref(&self) -> &[u8] {
		self.as_str().as_ref()
	}
}

impl<'a> TryFrom<&'a str> for &'a Number {
	type Error = InvalidNumber<&'a str>;

	#[inline]
	fn try_from(s: &'a str) -> Result<&'a Number, InvalidNumber<&'a str>> {
		Number::new(s)
	}
}

impl fmt::Display for Number {
	#[inline]
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.as_str().fmt(f)
	}
}

impl fmt::Debug for Number {
	#[inline]
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.as_str().fmt(f)
	}
}

/// JSON number buffer.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NumberBuf<B = String> {
	data: B,
}

impl<B> NumberBuf<B> {
	/// Creates a new number buffer by parsing the given input `data` buffer.
	#[inline]
	pub fn new(data: B) -> Result<Self, InvalidNumber<B>>
	where
		B: AsRef<[u8]>,
	{
		match Number::new(&data) {
			Ok(_) => Ok(NumberBuf { data }),
			Err(_) => Err(InvalidNumber(data)),
		}
	}

	/// Creates a new number buffer from the given input `data` buffer.
	///
	/// ## Safety
	///
	/// The input `data` **must** hold a valid JSON number string.
	#[inline]
	pub unsafe fn new_unchecked(data: B) -> Self {
		NumberBuf { data }
	}
}

impl<B: AsRef<[u8]>> FromStr for NumberBuf<B>
where
	B: for<'a> From<&'a str>,
{
	type Err = InvalidNumber<B>;

	#[inline]
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::new(s.into())
	}
}

impl<B: AsRef<[u8]>> Deref for NumberBuf<B> {
	type Target = Number;

	#[inline]
	fn deref(&self) -> &Number {
		unsafe { Number::new_unchecked(&self.data) }
	}
}

impl<B: AsRef<[u8]>> AsRef<str> for NumberBuf<B> {
	#[inline]
	fn as_ref(&self) -> &str {
		self.as_str()
	}
}

impl<B: AsRef<[u8]>> AsRef<[u8]> for NumberBuf<B> {
	#[inline]
	fn as_ref(&self) -> &[u8] {
		self.as_str().as_ref()
	}
}

impl<B: AsRef<[u8]>> fmt::Display for NumberBuf<B> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.as_str().fmt(f)
	}
}

impl<B: AsRef<[u8]>> fmt::Debug for NumberBuf<B> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.as_str().fmt(f)
	}
}

macro_rules! impl_from_int {
	($($ty:ty),*) => {
		$(
			impl<B: AsRef<[u8]> + for<'a> From<&'a str>> From<$ty> for NumberBuf<B> {
				#[inline]
				fn from(i: $ty) -> Self {
					unsafe {
						Self::new_unchecked(itoa::Buffer::new().format(i).into())
					}
				}
			}
		)*
	};
}

macro_rules! impl_from_float {
	($($ty:ty),*) => {
		$(
			impl<B: AsRef<[u8]> + for<'a> From<&'a str>> From<$ty> for NumberBuf<B> {
				#[inline]
				fn from(f: $ty) -> Self {
					unsafe {
						Self::new_unchecked(ryu::Buffer::new().format_finite(f).into())
					}
				}
			}
		)*
	};
}

impl_from_int!(u8, i8, u16, i16, u32, i32, u64, i64, usize, isize);
impl_from_float!(f32, f64);

#[cfg(test)]
mod tests {
	use super::*;

	macro_rules! positive_tests {
		{ $($id:ident: $input:literal),* } => {
			$(
				#[test]
				fn $id () {
					assert!(Number::new($input).is_ok())
				}
			)*
		};
	}

	macro_rules! negative_tests {
		{ $($id:ident: $input:literal),* } => {
			$(
				#[test]
				fn $id () {
					assert!(Number::new($input).is_err())
				}
			)*
		};
	}

	macro_rules! sign_tests {
		{ $($id:ident: $input:literal => $sign:ident),* } => {
			$(
				#[test]
				fn $id () {
					assert_eq!(Number::new($input).unwrap().sign(), Sign::$sign)
				}
			)*
		};
	}

	positive_tests! {
		pos_01: "0",
		pos_02: "-0",
		pos_03: "123",
		pos_04: "1.23",
		pos_05: "-12.34",
		pos_06: "12.34e+56",
		pos_07: "12.34E-56",
		pos_08: "0.0000"
	}

	negative_tests! {
		neg_01: "",
		neg_02: "00",
		neg_03: "01",
		neg_04: "-00",
		neg_05: "-01",
		neg_06: "0.000e+-1",
		neg_07: "12.34E-56abc",
		neg_08: "1.",
		neg_09: "12.34e",
		neg_10: "12.34e+",
		neg_11: "12.34E-"
	}

	sign_tests! {
		sign_zero_01: "0" => Zero,
		sign_zero_02: "-0" => Zero,
		sign_zero_03: "0.0" => Zero,
		sign_zero_04: "0.0e12" => Zero,
		sign_zero_05: "-0.0E-12" => Zero,
		sign_zero_06: "-0.00000" => Zero
	}

	sign_tests! {
		sign_pos_01: "1" => Positive,
		sign_pos_02: "0.1" => Positive,
		sign_pos_03: "0.01e23" => Positive,
		sign_pos_04: "1.0E-23" => Positive,
		sign_pos_05: "0.00001" => Positive
	}

	sign_tests! {
		sign_neg_01: "-1" => Negative,
		sign_neg_02: "-0.1" => Negative,
		sign_neg_03: "-0.01e23" => Negative,
		sign_neg_04: "-1.0E-23" => Negative,
		sign_neg_05: "-0.00001" => Negative
	}
}
