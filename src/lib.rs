extern crate byteorder;
#[macro_use] extern crate serde;
#[macro_use] extern crate serde_derive;

use byteorder::{BigEndian, ByteOrder};
use serde::de::{self, Deserialize, DeserializeSeed, Visitor, IntoDeserializer};

use std::{str, fmt, io};

macro_rules! enum_number {
    ($name:ident { $($variant:ident = $value:expr, )* }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name {
            $($variant = $value,)*
        }

        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: ::serde::Serializer
            {
                // Serialize the enum as a u64.
                serializer.serialize_u64(*self as u64)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: ::serde::Deserializer<'de>
            {
                struct Visitor;

                impl<'de> ::serde::de::Visitor<'de> for Visitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("positive integer")
                    }

                    fn visit_u64<E>(self, value: u64) -> Result<$name, E>
                        where E: ::serde::de::Error
                    {
                        // Rust does not come with a simple way of converting a
                        // number to an enum, so use a big `match`.
                        match value {
                            $( $value => Ok($name::$variant), )*
                            _ => Err(E::custom(
                                format!("unknown {} value: {}",
                                stringify!($name), value))),
                        }
                    }
                }

                // Deserialize the enum from a u64.
                deserializer.deserialize_u64(Visitor)
            }
        }
    }
}

#[repr(u16)]
enum_number!(TypeKind {
    I8 = 1,
    U8 = 2,
    I16 = 3,
    U16 = 4,
    I32 = 5,
    U32 = 6,
    I64 = 7,
    U64 = 8,
    String = 9,
    Timestamp = 10,
    Version = 11,
    Container = 12,
});

/*
#[derive(Debug, Clone)]
struct Type<'a> {
    code: [u8; 4],
    name: &'a str,
    kind: TypeKind,
}*/

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmapItem<'a, 'k> {
    name: Result<&'k str, [u8; 4]>,
    value: DmapValue<'a, 'k>
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DmapValue<'a, 'k> {
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    String(&'a str),
    Container(Vec<DmapItem<'a, 'k>>),
    Unknown(&'a [u8]),
}


#[derive(Deserialize, Debug, Clone)]
struct ContentCode<'a> {
    #[serde(rename = "dmap.contentcodesnumber", deserialize_with = "de_content_code")]
    code: [u8; 4],
    #[serde(borrow)]
    #[serde(rename = "dmap.contentcodesname")]
    name: &'a str,
    #[serde(rename = "dmap.contentcodestype")]
    kind: TypeKind,
}

fn de_content_code<'de, D>(d: D) -> Result<[u8; 4], D::Error>
    where D: de::Deserializer<'de>
{
    struct ContentCodeVisitor;
    impl<'de> de::Visitor<'de> for ContentCodeVisitor {
        type Value = [u8; 4];

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("content code (u32)")
        }

        fn visit_u32<E>(self, x: u32) -> Result<Self::Value, E> {
            let mut buf = [0; 4];
            BigEndian::write_u32(&mut buf, x);
            Ok(buf)
        }
    }
    d.deserialize_u32(ContentCodeVisitor)
}

#[derive(Deserialize, Debug, Clone)]
struct ContentCodesResponse<'a> {
    #[serde(rename = "dmap.status")]
    status: u32,
    #[serde(rename = "dmap.dictionary", borrow)]
    dictionary: Vec<ContentCode<'a>>,
}

fn de<'a, 'de, 'k: 'de, T>(v: &'a DmapItem<'de, 'k>) -> T
    where T: Deserialize<'de>
{
    T::deserialize(v).unwrap()
}

impl<'de, 'a, 'k: 'de> de::Deserializer<'de> for &'a DmapItem<'de, 'k> {
    type Error = serde::de::value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, serde::de::value::Error>
        where V: Visitor<'de>
    {
        match &self.value {
            &DmapValue::U16(x) => visitor.visit_u16(x),
            &DmapValue::U32(x) => visitor.visit_u32(x),
            &DmapValue::String(x) => visitor.visit_borrowed_str(x),
            _ => panic!("unimplemented {:#?}", self.value),
        }
    }

    fn deserialize_struct<V>(self,
                             _name: &'static str,
                             _fields: &'static [&'static str],
                             visitor: V) -> Result<V::Value, serde::de::value::Error>
        where V: Visitor<'de>
    {
        match &self.value {
            &DmapValue::Container(ref c) => visitor.visit_map(MapSeqVisitor {
                map: c.as_slice(),
                index: 0,
            }),
            _ => panic!("unimpl {:#?}", self.value),
        }
    }

    /*
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor<'de>
    {
        /*
        match &self.value {
            &DmapValue::Container(ref c) => visitor.visit_seq(MapSeqVisitor {
                map: c.as_slice(),
                index: 0,
            }),
            _ => unimplemented!(),
        }
         */
        unimplemented!();
    }*/

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor<'de>
    {
        /*
        use de::Error;
        println!("ignored_any");
        Err(Self::Error::custom("cslul"))
         */
        visitor.visit_none()
    }

    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string
        bytes byte_buf map unit newtype_struct enum seq
        unit_struct tuple_struct tuple option identifier
    }
}

struct MapSeqVisitor<'a, 'de: 'a, 'k: 'a> {
    map: &'a [DmapItem<'de, 'k>],
    index: usize,
}

impl<'a, 'de: 'a, 'k: 'a + 'de> de::MapAccess<'de> for MapSeqVisitor<'a, 'de, 'k> {
    type Error = serde::de::value::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
        where K: DeserializeSeed<'de>
    {
        if self.index == self.map.len() {
            Ok(None)
        } else {
            let name: &'k str = self.map[self.index].name.unwrap();
            //println!("yielding {}", name);
            self.index += 1;
            seed.deserialize(name.into_deserializer()).map(Some)
        }
    }

    fn next_value_seed<K>(&mut self, seed: K) -> Result<K::Value, Self::Error>
        where K: DeserializeSeed<'de>
    {
        let element = &self.map[self.index - 1];
        let seqlen = self.map.iter().skip(self.index).take_while(|x| x.name == element.name).count();
        if seqlen > 0 {
            // seq detected (ugly hack but whatever)
            let seq = &self.map[self.index-1..][..seqlen+1];
            self.index += seqlen;
            seed.deserialize(SeqDeserializer { items: seq })
        } else {
            seed.deserialize(&self.map[self.index - 1])
        }
    }
}

impl<'a, 'de: 'a, 'k: 'a + 'de> de::SeqAccess<'de> for MapSeqVisitor<'a, 'de, 'k> {
    type Error = serde::de::value::Error;

    fn next_element_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
        where K: DeserializeSeed<'de>
    {
        if self.index == self.map.len() {
            Ok(None)
        } else {
            self.index += 1;
            seed.deserialize(&self.map[self.index - 1]).map(Some)
        }
    }
}

struct SeqDeserializer<'a, 'de: 'a, 'k: 'a> {
    items: &'a [DmapItem<'de, 'k>],
}

impl<'a, 'de: 'a, 'k: 'de + 'a> de::Deserializer<'de> for SeqDeserializer<'a, 'de, 'k> {
    type Error = serde::de::value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, serde::de::value::Error>
        where V: Visitor<'de>
    {
        visitor.visit_seq(MapSeqVisitor {
            map: self.items,
            index: 0,
        })
    }

    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string
        bytes byte_buf map unit newtype_struct seq struct ignored_any
        unit_struct tuple_struct tuple option identifier enum
    }
}

use std::borrow::Cow;

pub struct Parser<'names> {
    types: Cow<'names, [ContentCode<'names>]>,
}

static BOOTSTRAP_TYPES: &'static [ContentCode<'static>] = &[
    ContentCode { code: *b"mccr", name: "dmap.contentcodesresponse", kind: TypeKind::Container },
    ContentCode { code: *b"mstt", name: "dmap.status", kind: TypeKind::U32 },
    ContentCode { code: *b"mdcl", name: "dmap.dictionary", kind: TypeKind::Container },
    ContentCode { code: *b"mcnm", name: "dmap.contentcodesnumber", kind: TypeKind::U32 },
    ContentCode { code: *b"mcna", name: "dmap.contentcodesname", kind: TypeKind::String },
    ContentCode { code: *b"mcty", name: "dmap.contentcodestype", kind: TypeKind::U16 },
];

impl<'names> Parser<'names> {
    pub fn new(content_codes: &'names [u8]) -> Parser<'names> {
        let mut parser = Parser {
            types: Cow::Borrowed(BOOTSTRAP_TYPES),
        };
        let ccs = parser.parse(content_codes).unwrap();
        // fixme: convert asserts to errors
        //println!("{:#?}", ccs);

        assert_eq!(ccs.name, Ok("dmap.contentcodesresponse"));
        let ccs: ContentCodesResponse = de(&ccs);
        //println!("{:#?}", ccs);

        assert_eq!(ccs.status, 200);
        parser.types = Cow::Owned(ccs.dictionary);

        parser
    }

    pub fn parse<'a>(&self, data: &'a [u8]) -> io::Result<DmapItem<'a, 'names>> {
        self.do_parse(data).and_then(|(x, t)| if t.is_empty() { Ok(x) } else { Err(io::Error::new(io::ErrorKind::Other, "cslul")) })
    }

    fn do_parse<'a>(&self, data: &'a [u8]) -> io::Result<(DmapItem<'a, 'names>, &'a [u8])> {
        let mut code = [0; 4];
        code.copy_from_slice(&data[0..4]);
        let size = BigEndian::read_u32(&data[4..8]) as usize;
        //println!("handling {}", typ.name);
        //println!("{} vs {}", data.len(), 8+size);
        let body = &data[8..8+size];
        let tail = &data[8+size..];

        let item = match self.types.iter().find(|x| x.code == code) {
            Some(typ) => DmapItem {
                name: Ok(typ.name),
                value: match typ.kind {
                    TypeKind::I8 => DmapValue::I8(body[0] as i8),
                    TypeKind::U8 => DmapValue::U8(body[0]),
                    TypeKind::I16 => DmapValue::I16(BigEndian::read_i16(body)),
                    TypeKind::U16 => DmapValue::U16(BigEndian::read_u16(body)),
                    TypeKind::I32 => DmapValue::I32(BigEndian::read_i32(body)),
                    TypeKind::U32 | TypeKind::Timestamp /*fixme*/ => DmapValue::U32(BigEndian::read_u32(body)),
                    TypeKind::I64 => DmapValue::I64(BigEndian::read_i64(body)),
                    TypeKind::U64 => DmapValue::U64(BigEndian::read_u64(body)),
                    TypeKind::String => DmapValue::String(str::from_utf8(body).unwrap()), // fixme unwrap
                    TypeKind::Container => {
                        let mut values = Vec::new();
                        let mut todo = body;
                        while !todo.is_empty() {
                            let (v, t) = self.do_parse(todo)?;
                            values.push(v);
                            todo = t;
                        }
                        DmapValue::Container(values)
                    }
                    _ => panic!("unimpl {:?}", typ.kind),
                }
            },
            None => DmapItem {
                name: Err(code),
                value: DmapValue::Unknown(body),
            },
        };

        Ok((item, tail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_codes() {
        Parser::new(include_bytes!("../testdata/content-codes.bin"));
    }

    #[test]
    fn login() {
        let parser = Parser::new(include_bytes!("../testdata/content-codes.bin"));
        let login = parser.parse(include_bytes!("../testdata/login.bin")).unwrap();
        println!("{:#?}", login);
        assert_eq!(login.name, Ok("dmap.loginresponse"));
        match login.value {
            DmapValue::Container(c) => {
                assert_eq!(c[0].name, Ok("dmap.status"));
                assert_eq!(c[0].value, DmapValue::I32(200));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn items() {
        // items.bin is not in the repo
        // so expect this to fail unless you provide your own copy
        use std::fs::File;
        let parser = Parser::new(include_bytes!("../testdata/content-codes.bin"));
        let mut items = Vec::new();
        File::open("testdata/items.bin").unwrap().read_to_end(&mut items).unwrap();
        let items = parser.parse(items.as_slice()).unwrap();
        println!("{:?}", items); // just parsing this is already impressive
    }
}
