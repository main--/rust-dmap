extern crate byteorder;
#[macro_use] extern crate serde;
#[macro_use] extern crate serde_derive;

use byteorder::{BigEndian, ByteOrder};

use std::{str, fmt};
use std::borrow::Cow;

#[macro_use] mod enum_number;

pub mod de;
pub mod ser;
pub mod value;

pub use value::{DmapValue, DmapItem};
pub use de::{from_slice, Deserializer};
pub use ser::{to_vec, Serializer};

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



#[derive(Serialize, Deserialize, Debug, Clone)]
struct ContentCode<'a> {
    #[serde(rename = "dmap.contentcodesnumber", deserialize_with = "de_content_code", serialize_with = "ser_content_code")]
    code: [u8; 4],
    #[serde(borrow)]
    #[serde(rename = "dmap.contentcodesname")]
    name: &'a str,
    #[serde(rename = "dmap.contentcodestype")]
    kind: TypeKind,
}

fn de_content_code<'de, D>(d: D) -> Result<[u8; 4], D::Error>
    where D: serde::de::Deserializer<'de>
{
    struct ContentCodeVisitor;
    impl<'de> serde::de::Visitor<'de> for ContentCodeVisitor {
        type Value = [u8; 4];

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("content code (u32)")
        }

        fn visit_u32<E>(self, x: u32) -> Result<Self::Value, E> {
            let mut buf = [0; 4];
            BigEndian::write_u32(&mut buf, x);
            Ok(buf)
        }

        fn visit_i32<E>(self, x: i32) -> Result<Self::Value, E> {
            let mut buf = [0; 4]; // KILL ME
            BigEndian::write_i32(&mut buf, x);
            Ok(buf)
        }
    }
    d.deserialize_u32(ContentCodeVisitor)
}

fn ser_content_code<S>(code: &[u8; 4], s: S) -> Result<S::Ok, S::Error>
    where S: serde::ser::Serializer
{
    serde::ser::Serialize::serialize(&BigEndian::read_i32(code), s)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ContentCodesResponse<'a> {
    #[serde(rename = "dmap.status")]
    status: i32,
    #[serde(rename = "dmap.dictionary", borrow)]
    dictionary: Vec<ContentCode<'a>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ContentCodesResponseWrapper<'a> {
    #[serde(rename = "dmap.contentcodesresponse", borrow)]
    inner: ContentCodesResponse<'a>
}

pub struct Parser<'names> {
    types: Cow<'names, [ContentCode<'names>]>,
}

static BOOTSTRAP_TYPES: &'static [ContentCode<'static>] = &[
    ContentCode { code: *b"mccr", name: "dmap.contentcodesresponse", kind: TypeKind::Container },
    ContentCode { code: *b"mstt", name: "dmap.status", kind: TypeKind::I32 },
    ContentCode { code: *b"mdcl", name: "dmap.dictionary", kind: TypeKind::Container },
    ContentCode { code: *b"mcnm", name: "dmap.contentcodesnumber", kind: TypeKind::U32 },
    ContentCode { code: *b"mcna", name: "dmap.contentcodesname", kind: TypeKind::String },
    ContentCode { code: *b"mcty", name: "dmap.contentcodestype", kind: TypeKind::I16 },
];

impl<'names> Parser<'names> {
    pub fn new(content_codes: &'names [u8]) -> Parser<'names> {
        let mut parser = Parser {
            types: Cow::Borrowed(BOOTSTRAP_TYPES),
        };

        let ccsw: ContentCodesResponseWrapper = de::from_slice(&parser, content_codes).unwrap();

        let mut ccs = ccsw.inner;
        assert_eq!(ccs.status, 200);

        //println!("{:#?}", ccs);

        // fix these because apple gave them a wrong type (???wtf???)
        // FIXME develop a better solution
        ccs.dictionary.iter_mut().find(|x| x.name == "dmap.editcommandssupported").unwrap().kind = TypeKind::I16;
        ccs.dictionary.iter_mut().find(|x| x.name == "dmap.authenticationschemes").unwrap().kind = TypeKind::I8;
        ccs.dictionary.iter_mut().find(|x| x.name == "com.apple.itunes.itms-playlistid").unwrap().kind = TypeKind::I64;
        ccs.dictionary.iter_mut().find(|x| x.name == "com.apple.itunes.rental-pb-start").unwrap().kind = TypeKind::String;
        ccs.dictionary.iter_mut().find(|x| x.name == "dmap.itemdateplayed").unwrap().kind = TypeKind::I32;
        parser.types = Cow::Owned(ccs.dictionary);

        parser
    }

    #[cfg(test)]
    fn old_parse<'a>(&self, data: &'a [u8]) -> DmapItem<'a, 'names> {
        let (x, t) = self.old_do_parse(data);
        assert!(t.is_empty());
        x
    }

    #[cfg(test)]
    fn old_do_parse<'a>(&self, data: &'a [u8]) -> (DmapItem<'a, 'names>, &'a [u8]) {
        use value::ItemName;

        let mut code = [0; 4];
        code.copy_from_slice(&data[0..4]);
        let size = BigEndian::read_u32(&data[4..8]) as usize;

        let body = &data[8..8+size];
        let tail = &data[8+size..];

        let item = match self.types.iter().find(|x| x.code == code) {
            Some(typ) => DmapItem {
                name: ItemName::Name(typ.name),
                value: match typ.kind {
                    TypeKind::I8 => DmapValue::I8(body[0] as i8),
                    TypeKind::U8 => DmapValue::U8(body[0]),
                    TypeKind::I16 => DmapValue::I16(BigEndian::read_i16(body)),
                    TypeKind::U16 => DmapValue::U16(BigEndian::read_u16(body)),
                    TypeKind::I32 => DmapValue::I32(BigEndian::read_i32(body)),
                    TypeKind::U32 | TypeKind::Timestamp | TypeKind::Version /*fixme*/
                        => DmapValue::U32(BigEndian::read_u32(body)),
                    TypeKind::I64 => DmapValue::I64(BigEndian::read_i64(body)),
                    TypeKind::U64 => DmapValue::U64(BigEndian::read_u64(body)),
                    TypeKind::String => DmapValue::String(str::from_utf8(body).unwrap()), // fixme unwrap
                    TypeKind::Container => {
                        let mut values = Vec::new();
                        let mut todo = body;
                        while !todo.is_empty() {
                            let (v, t) = self.old_do_parse(todo);
                            values.push(v);
                            todo = t;
                        }
                        DmapValue::Container(values)
                    }
                }
            },
            None => DmapItem {
                name: ItemName::Code(code),
                value: DmapValue::Unknown(body),
            },
        };

        (item, tail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use value::ItemName;

    fn verify_parse<'a, 'b, 'k: 'b>(parser: &'a Parser<'k>, data: &'b [u8]) -> DmapItem<'b, 'b> {
        let val1: DmapValue = de::from_slice(parser, data).unwrap();
        let val2 = parser.old_parse(data);

        let data2 = ser::to_vec(&parser, &val1).unwrap();
        assert_eq!(data.len(), data2.len());
        assert_eq!(data, data2.as_slice());
        let val3: DmapValue = de::from_slice(&parser, data2.as_slice()).unwrap();
        assert_eq!(val1, val3);

        let val1 = match val1 {
            DmapValue::Container(v) => v,
            _ => unreachable!(),
        };
        assert_eq!(val1.len() , 1);


        //println!("{:#?} vs {:#?}", val1[0], val2);
        assert_eq!(val1[0], val2);
        val1.into_iter().next().unwrap()
    }

    #[test]
    fn content_codes() {
        let ccs = include_bytes!("../testdata/content-codes.bin");
        let parser = Parser::new(ccs);
        verify_parse(&parser, ccs);
    }

    #[test]
    fn serverinfo() {
        let parser = Parser::new(include_bytes!("../testdata/content-codes.bin"));
        verify_parse(&parser, include_bytes!("../testdata/server-info.bin"));
    }

    #[test]
    fn login() {
        let parser = Parser::new(include_bytes!("../testdata/content-codes.bin"));
        let login = verify_parse(&parser, include_bytes!("../testdata/login.bin"));
        println!("{:#?}", login);
        assert_eq!(login.name, ItemName::Name("dmap.loginresponse"));
        match login.value {
            DmapValue::Container(c) => {
                assert_eq!(c[0].name, ItemName::Name("dmap.status"));
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
        use std::io::Read;
        let parser = Parser::new(include_bytes!("../testdata/content-codes.bin"));
        let mut items = Vec::new();
        File::open("testdata/items.bin").unwrap().read_to_end(&mut items).unwrap();
        let items = verify_parse(&parser, items.as_slice());
        println!("{:?}", items); // just parsing this is already impressive
    }
}
