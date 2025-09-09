use crate::{Buffer, CowNumber, InvalidNumber, Number, NumberBuf};
use de::{Deserialize, Deserializer};
use ser::{Serialize, Serializer};
use serde::{
	de::{self, DeserializeSeed},
	forward_to_deserialize_any, ser,
};
use std::{fmt, marker::PhantomData};

/// Structure name used to serialize number with arbitrary precision.
///
/// This is the same used by `serde_json`, to ensure compatibility with this
/// crate.
const TOKEN: &str = "$serde_json::private::Number";

impl Serialize for Number {
	#[inline]
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		if let Some(v) = self.as_i64() {
			serializer.serialize_i64(v)
		} else if let Some(v) = self.as_u64() {
			serializer.serialize_u64(v)
		} else {
			use serde::ser::SerializeStruct;
			let mut s = serializer.serialize_struct(TOKEN, 1)?;
			s.serialize_field(TOKEN, self.as_str())?;
			s.end()
		}
	}
}

impl<B: Buffer> Serialize for NumberBuf<B> {
	#[inline]
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		self.as_number().serialize(serializer)
	}
}

impl<'de, B: Buffer> Deserialize<'de> for NumberBuf<B> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer
			.deserialize_any(NumberVisitor(PhantomData))
			.map(CowNumber::into_owned)
	}
}

impl<'de, B: Buffer> Deserialize<'de> for CowNumber<'de, B> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer.deserialize_any(NumberVisitor(PhantomData))
	}
}

/// Number visitor.
pub struct NumberVisitor<B>(PhantomData<B>);

impl<B> NumberVisitor<B> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<'de, B: Buffer> de::Visitor<'de> for NumberVisitor<B> {
	type Value = CowNumber<'de, B>;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		formatter.write_str("JSON number")
	}

	#[inline]
	fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
		Ok(CowNumber::Owned(value.into()))
	}

	#[inline]
	fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
		Ok(CowNumber::Owned(value.into()))
	}

	#[inline]
	fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
		NumberBuf::try_from(value)
			.map(CowNumber::Owned)
			.map_err(|_| E::invalid_value(de::Unexpected::Float(value), &self))
	}

	fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
	where
		A: de::MapAccess<'de>,
	{
		struct Key;

		impl<'de> Deserialize<'de> for Key {
			fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
			where
				D: Deserializer<'de>,
			{
				struct KeyVisitor;

				impl<'de> de::Visitor<'de> for KeyVisitor {
					type Value = Key;

					fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
						formatter.write_str("a valid number field")
					}

					fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
					where
						E: de::Error,
					{
						if v == TOKEN {
							Ok(Key)
						} else {
							Err(serde::de::Error::custom("expected field with custom name"))
						}
					}
				}

				deserializer.deserialize_identifier(KeyVisitor)
			}
		}

		struct Value<'de, B>(CowNumber<'de, B>);

		impl<'de, B: Buffer> Deserialize<'de> for Value<'de, B> {
			fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
			where
				D: Deserializer<'de>,
			{
				struct ValueVisitor<B>(PhantomData<B>);

				impl<'de, B: Buffer> de::Visitor<'de> for ValueVisitor<B> {
					type Value = Value<'de, B>;

					fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
						formatter.write_str("string containing a JSON number")
					}

					fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
					where
						E: de::Error,
					{
						match Number::new(v) {
							Ok(v) => Ok(Value(CowNumber::Borrowed(v))),
							Err(InvalidNumber(_)) => {
								Err(de::Error::custom(InvalidNumber(v.to_owned())))
							}
						}
					}

					fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
					where
						E: de::Error,
					{
						self.visit_string(v.to_owned())
					}

					fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
					where
						E: de::Error,
					{
						match NumberBuf::new(B::from_vec(v.into_bytes())) {
							Ok(v) => Ok(Value(CowNumber::Owned(v))),
							Err(InvalidNumber(bytes)) => Err(de::Error::custom(InvalidNumber(
								String::from_utf8(bytes.as_ref().to_owned()).unwrap(),
							))),
						}
					}
				}

				deserializer.deserialize_identifier(ValueVisitor(PhantomData))
			}
		}

		match map.next_key()? {
			Some(Key) => {
				let value: Value<'de, B> = map.next_value()?;
				Ok(value.0)
			}
			None => Err(de::Error::invalid_type(de::Unexpected::Map, &self)),
		}
	}
}

/// Unexpected value that is not a number.
#[derive(Debug)]
pub struct Unexpected(String);

impl fmt::Display for Unexpected {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

impl std::error::Error for Unexpected {}

impl de::Error for Unexpected {
	fn custom<T>(msg: T) -> Self
	where
		T: fmt::Display,
	{
		Self(msg.to_string())
	}

	fn invalid_type(unexp: de::Unexpected, exp: &dyn de::Expected) -> Self {
		if let de::Unexpected::Unit = unexp {
			Self::custom(format_args!("invalid type: null, expected {}", exp))
		} else {
			Self::custom(format_args!("invalid type: {}, expected {}", unexp, exp))
		}
	}
}

impl<'de, B: Buffer> Deserializer<'de> for NumberBuf<B> {
	type Error = Unexpected;

	#[inline(always)]
	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: serde::de::Visitor<'de>,
	{
		NumberDeserializer::new(CowNumber::Owned(self)).deserialize_any(visitor)
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf option unit unit_struct seq tuple
		tuple_struct map struct newtype_struct enum identifier ignored_any
	}
}

impl<'de, B: Buffer> Deserializer<'de> for &'de NumberBuf<B> {
	type Error = Unexpected;

	#[inline(always)]
	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: serde::de::Visitor<'de>,
	{
		self.as_number().deserialize_any(visitor)
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf option unit unit_struct seq tuple
		tuple_struct map struct newtype_struct enum identifier ignored_any
	}
}

impl<'de> Deserializer<'de> for &'de Number {
	type Error = Unexpected;

	#[inline(always)]
	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: serde::de::Visitor<'de>,
	{
		NumberDeserializer::new(CowNumber::<Vec<u8>>::Borrowed(self)).deserialize_any(visitor)
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf option unit unit_struct seq tuple
		tuple_struct map struct newtype_struct enum identifier ignored_any
	}
}

pub struct NumberDeserializer<'de, B, E>(CowNumber<'de, B>, PhantomData<E>);

impl<'de, B, E> NumberDeserializer<'de, B, E> {
	pub fn new(number: CowNumber<'de, B>) -> Self {
		Self(number, PhantomData)
	}
}

impl<'de, B: Buffer, E: serde::de::Error> Deserializer<'de> for NumberDeserializer<'de, B, E> {
	type Error = E;

	#[inline(always)]
	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: serde::de::Visitor<'de>,
	{
		// if let Some(u) = self.as_u64() {
		// 	visitor.visit_u64(u)
		// } else if let Some(i) = self.as_i64() {
		// 	visitor.visit_i64(i)
		// } else {
		// 	visitor.visit_f64(self.as_f64_lossy())
		// }
		todo!()
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf option unit unit_struct seq tuple
		tuple_struct map struct newtype_struct enum identifier ignored_any
	}
}

pub struct NumberAsMapAccess<'de, B, E>(CowNumber<'de, B>, PhantomData<E>);

impl<'de, B, E> NumberAsMapAccess<'de, B, E> {
	pub fn new(number: CowNumber<'de, B>) -> Self {
		Self(number, PhantomData)
	}
}

impl<'de, B: Buffer, E: serde::de::Error> serde::de::MapAccess<'de>
	for NumberAsMapAccess<'de, B, E>
{
	type Error = E;

	fn size_hint(&self) -> Option<usize> {
		todo!()
	}

	fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
	where
		K: DeserializeSeed<'de>,
	{
		todo!()
	}

	fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
	where
		V: DeserializeSeed<'de>,
	{
		todo!()
	}
}
