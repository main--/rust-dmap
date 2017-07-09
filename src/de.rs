use super::{Parser, ContentCode, TypeKind};
use byteorder::{BigEndian, ByteOrder};
use serde::de::{self, Error as ErrorTrait, Visitor, DeserializeSeed, IntoDeserializer, Deserialize};
use serde::de::value::{BorrowedBytesDeserializer, BorrowedStrDeserializer, MapAccessDeserializer};
use std::{str, mem};

pub struct MapDeserializer<'a, 'k: 'a, 'de>  {
    parser: &'a Parser<'k>,
    current: Option<RawMessage<'a, 'k, 'de>>,
    tail: &'de [u8],
}

struct RawMessage<'a, 'k: 'a, 'de> {
    typedesc: Result<&'a ContentCode<'k>, &'de [u8]>,
    body: &'de [u8],
}
type Error = de::value::Error;

impl<'a, 'k: 'a + 'de, 'de: 'a> RawMessage<'a, 'k, 'de> {
    fn code(&self) -> [u8; 4] {
        let code = match &self.typedesc {
            &Ok(ref c) => &c.code,
            &Err(x) => x,
        };
        let mut buf = [0; 4];
        buf.copy_from_slice(code);
        buf
    }
}

impl<'a, 'k: 'a + 'de, 'de> MapDeserializer<'a, 'k, 'de> {
    pub fn new(parser: &'a Parser<'k>, input: &'de [u8]) -> MapDeserializer<'a, 'k, 'de> {
        MapDeserializer {
            parser,
            tail: input,
            current: None,
        }
    }

    fn next_message(&mut self) -> Result<Option<RawMessage<'a, 'k, 'de>>, Error> {
        Ok(match self.current.take() {
            Some(x) => Some(x),
            None if !self.tail.is_empty() => {
                let err = Error::custom("Failed to get message (truncated input?)");
                let input = self.tail;
                let mut code = [0; 4];
                let code_ref = input.get(0..4).ok_or(err.clone())?;
                code.copy_from_slice(code_ref);
                let size = BigEndian::read_u32(input.get(4..8).ok_or(err.clone())?) as usize;
                let body = input.get(8..size+8).ok_or(err)?;
                self.tail = &input[8+size..];
                let typedesc = self.parser.types.iter().find(|x| x.code == code).ok_or(code_ref);
                Some(RawMessage { typedesc, body })
            }
            None => None,
        })
    }
}


impl<'de, 'a, 'k: 'a + 'de, 'b> de::MapAccess<'de> for &'b mut MapDeserializer<'a, 'k, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
        where K: DeserializeSeed<'de>
    {
        let msg = match self.next_message()? {
            Some(x) => x,
            None => return Ok(None),
        };

        let name = msg.typedesc.map(|c| c.name);
        println!("yielding {:?}", name);
        self.current = Some(msg);

        match name {
            Ok(s) => seed.deserialize(BorrowedStrDeserializer::new(s)),
            Err(c) => seed.deserialize(BorrowedBytesDeserializer::new(c)),
        }.map(Some)
    }

    fn next_value_seed<K>(&mut self, seed: K) -> Result<K::Value, Self::Error>
        where K: DeserializeSeed<'de>
    {
        seed.deserialize(ValueDeserializer(self))
    }
}

struct ValueDeserializer<'a: 'b, 'k: 'a, 'de: 'b, 'b>(&'b mut MapDeserializer<'a, 'k, 'de>);

impl<'a: 'b, 'k: 'a + 'de, 'de: 'b, 'b> de::Deserializer<'de> for ValueDeserializer<'a, 'k, 'de, 'b> {
    type Error = Error;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf unit unit_struct newtype_struct tuple
        tuple_struct map struct enum identifier ignored_any
    }

    fn deserialize_any<V>(self, v: V) -> Result<V::Value, Self::Error>
        where V: Visitor<'de>
    {
        let RawMessage { typedesc, body } = self.0.current.take().unwrap();
        println!("dany {:?}", typedesc);
        //println!("{:?} {}", typedesc, body.len());
        match typedesc {
            Ok(c) => match c.kind {
                TypeKind::I8 => v.visit_i8(body[0] as i8),
                TypeKind::U8 => v.visit_u8(body[0]),
                TypeKind::I16 => v.visit_i16(BigEndian::read_i16(body)),
                TypeKind::U16 => v.visit_u16(BigEndian::read_u16(body)),
                TypeKind::I32 => v.visit_i32(BigEndian::read_i32(body)),
                TypeKind::U32 | TypeKind::Timestamp | TypeKind::Version /*fixme*/
                    => v.visit_u32(BigEndian::read_u32(body)),
                TypeKind::I64 => v.visit_i64(BigEndian::read_i64(body)),
                TypeKind::U64 => v.visit_u64(BigEndian::read_u64(body)),
                TypeKind::String => v.visit_borrowed_str(str::from_utf8(body).map_err(|_| Error::custom("invalid utf8 lul"))?), // FIXME
                TypeKind::Container => v.visit_map(&mut MapDeserializer {
                    parser: self.0.parser,
                    current: None,
                    tail: body,
                }),
            },
            Err(_) => v.visit_borrowed_bytes(body),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor<'de>
    {
        visitor.visit_some(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor<'de>
    {
        let code = self.0.current.as_ref().unwrap().code();
        visitor.visit_seq(SeqDeserializer { code, parent: self.0 })
    }
}

struct SeqDeserializer<'a: 'b, 'k: 'a, 'de: 'b, 'b> {
    parent: &'b mut MapDeserializer<'a, 'k, 'de>,
    code: [u8; 4],
}

impl<'de, 'a, 'k: 'a + 'de, 'b> de::SeqAccess<'de> for SeqDeserializer<'a, 'k, 'de, 'b> {
    type Error = Error;

    fn next_element_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
        where K: DeserializeSeed<'de>
    {
        let msg = match self.parent.next_message()? {
            Some(x) => x,
            None => return Ok(None),
        };

        if msg.code() == self.code {
            seed.deserialize(MapAccessDeserializer::new(
                &mut MapDeserializer::new(self.parent.parser, msg.body))).map(Some)
        } else {
            self.parent.current = Some(msg);
            Ok(None)
        }
    }
}


pub fn from_slice<'a, 'k: 'a + 'de, 'de, T>(parser: &'a Parser<'k>, b: &'de [u8]) -> Result<T, Error>
    where T: Deserialize<'de>
{
    let mut deserializer = MapDeserializer::new(parser, b);
    let t = T::deserialize(MapAccessDeserializer::new(&mut deserializer))?;
    if deserializer.tail.is_empty() {
        Ok(t)
    } else {
        Err(Error::custom("trailing data"))
    }
}
