use super::{Parser, ContentCode, TypeKind};
use byteorder::{BigEndian, ByteOrder};
use serde::de::{self, Error as ErrorTrait, Visitor, DeserializeSeed, IntoDeserializer, Deserialize};
use serde::de::value::{BorrowedBytesDeserializer, BorrowedStrDeserializer};
use std::{str, mem};

pub struct Deserializer<'a, 'k: 'a, 'de>  {
    parser: &'a Parser<'k>,
    current: Option<RawMessage<'a, 'k, 'de>>,
    tail: &'de [u8],
}

struct RawMessage<'a, 'k: 'a, 'de> {
    typedesc: Result<&'a ContentCode<'k>, &'de [u8]>,
    body: &'de [u8],
}
type Error = de::value::Error;

impl<'a, 'k: 'a + 'de, 'de> Deserializer<'a, 'k, 'de> {
    pub fn new(parser: &'a Parser<'k>, input: &'de [u8]) -> Deserializer<'a, 'k, 'de> {
        Deserializer {
            parser,
            tail: input,
            current: None,
        }
    }

    fn next_message(&mut self) -> Result<RawMessage<'a, 'k, 'de>, Error> {
        let err = Error::custom("Failed to get message (truncated input?)");
        let input = self.tail;
        let mut code = [0; 4];
        let code_ref = input.get(0..4).ok_or(err.clone())?;
        code.copy_from_slice(code_ref);
        let size = BigEndian::read_u32(input.get(4..8).ok_or(err.clone())?) as usize;
        let body = input.get(8..size+8).ok_or(err)?;
        self.tail = &input[8+size..];
        let typedesc = self.parser.types.iter().find(|x| x.code == code).ok_or(code_ref);
        Ok(RawMessage { typedesc, body })
    }
}

impl<'de, 'a, 'k: 'a + 'de, 'b> de::Deserializer<'de> for &'b mut Deserializer<'a, 'k, 'de> {
    type Error = Error;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf option unit unit_struct newtype_struct tuple seq
        tuple_struct map struct enum identifier ignored_any
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor<'de>
    {
        visitor.visit_map(self)
    }
}

impl<'de, 'a, 'k: 'a + 'de, 'b> de::MapAccess<'de> for &'b mut Deserializer<'a, 'k, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
        where K: DeserializeSeed<'de>
    {
        let msg = match self.current.take() {
            None => {
                if self.tail.is_empty() {
                    return Ok(None);
                }

                self.next_message()?
            }
            Some(x) => x,
        };

        let name = msg.typedesc.map(|c| c.name);
        //println!("yielding {:?}", name);
        self.current = Some(msg);

        match name {
            Ok(s) => seed.deserialize(BorrowedStrDeserializer::new(s)),
            Err(c) => seed.deserialize(BorrowedBytesDeserializer::new(c)),
        }.map(Some)
    }

    fn next_value_seed<K>(&mut self, seed: K) -> Result<K::Value, Self::Error>
        where K: DeserializeSeed<'de>
    {
        let RawMessage { typedesc, body } = self.current.take().unwrap();
        //println!("{:?} {}", typedesc, body.len());
        match typedesc {
            Ok(c) => match c.kind {
                TypeKind::I8 => seed.deserialize((body[0] as i8).into_deserializer()),
                TypeKind::U8 => seed.deserialize(body[0].into_deserializer()),
                TypeKind::I16 => seed.deserialize(BigEndian::read_i16(body).into_deserializer()),
                TypeKind::U16 => seed.deserialize(BigEndian::read_u16(body).into_deserializer()),
                TypeKind::I32 => seed.deserialize(BigEndian::read_i32(body).into_deserializer()),
                TypeKind::U32 | TypeKind::Timestamp | TypeKind::Version /*fixme*/
                    => seed.deserialize(BigEndian::read_u32(body).into_deserializer()),
                TypeKind::I64 => seed.deserialize(BigEndian::read_i64(body).into_deserializer()),
                TypeKind::U64 => seed.deserialize(BigEndian::read_u64(body).into_deserializer()),
                TypeKind::String => seed.deserialize(BorrowedStrDeserializer::new(str::from_utf8(body).map_err(|_| Error::custom("invalid utf8 lul"))?)), // FIXME
                TypeKind::Container => {
                    seed.deserialize(ContainerDeserializer {
                        parent: self,
                        code: c.code,
                        body: Some(body),
                    })
                }
            },
            Err(_) => seed.deserialize(BorrowedBytesDeserializer::new(body)),
        }
    }
}

struct ContainerDeserializer<'a: 'b, 'k: 'a, 'de: 'b, 'b> {
    parent: &'b mut Deserializer<'a, 'k, 'de>,
    code: [u8; 4],
    body: Option<&'de [u8]>,
}

impl<'a: 'b, 'k: 'a + 'de, 'de: 'b, 'b> de::Deserializer<'de> for ContainerDeserializer<'a, 'k, 'de, 'b> {
    type Error = Error;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf option unit unit_struct newtype_struct tuple
        tuple_struct map struct enum identifier ignored_any
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor<'de>
    {
        visitor.visit_map(&mut Deserializer {
            parser: self.parent.parser,
            current: None,
            tail: self.body.unwrap(),
        })
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor<'de>
    {
        visitor.visit_seq(self)
    }
}

impl<'de, 'a, 'k: 'a + 'de, 'b> de::SeqAccess<'de> for ContainerDeserializer<'a, 'k, 'de, 'b> {
    type Error = Error;
    fn next_element_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
        where K: DeserializeSeed<'de>
    {
        let replacement = if self.parent.tail.is_empty() {
            None
        } else {
            let msg = self.parent.next_message()?;
            if msg.typedesc.map(|t| t.code) == Ok(self.code) {
                Some(msg.body)
            } else {
                self.parent.current = Some(msg);
                None
            }
        };

        match mem::replace(&mut self.body, replacement) {
            None => Ok(None),
            Some(b) => seed.deserialize(&mut Deserializer {
                parser: self.parent.parser,
                current: None,
                tail: b
            }).map(Some),
        }
    }
}

pub fn from_slice<'a, 'k: 'a + 'de, 'de, T>(parser: &'a Parser<'k>, b: &'de [u8]) -> Result<T, Error>
    where T: Deserialize<'de>
{
    let mut deserializer = Deserializer::new(parser, b);
    let t = T::deserialize(&mut deserializer)?;
    if deserializer.tail.is_empty() {
        Ok(t)
    } else {
        Err(Error::custom("trailing data"))
    }
}
