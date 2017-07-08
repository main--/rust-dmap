use serde::de;
use serde::ser::{self, SerializeMap};

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmapItem<'a, 'k> {
    pub name: ItemName<'k>,
    pub value: DmapValue<'a, 'k>
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemName<'k> {
    Name(&'k str),
    Code([u8; 4]),
}

impl<'de> de::Deserialize<'de> for ItemName<'de> {
    fn deserialize<D>(deserializer: D) -> Result<ItemName<'de>, D::Error>
        where D: de::Deserializer<'de>
    {
        struct ItemNameVisitor;

        impl<'de> de::Visitor<'de> for ItemNameVisitor {
            type Value = ItemName<'de>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid item name")
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
                Ok(ItemName::Name(value))
            }

            fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Self::Value, E> {
                let mut buf = [0; 4];
                buf.copy_from_slice(value);
                Ok(ItemName::Code(buf))
            }
        }

        deserializer.deserialize_any(ItemNameVisitor)
    }
}


impl<'de> de::Deserialize<'de> for DmapValue<'de, 'de> {
    fn deserialize<D>(deserializer: D) -> Result<DmapValue<'de, 'de>, D::Error>
        where D: de::Deserializer<'de>
    {
        struct ValueVisitor;

        impl<'de> de::Visitor<'de> for ValueVisitor {
            type Value = DmapValue<'de, 'de>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid DMAP value")
            }

            fn visit_i8<E>(self, value: i8) -> Result<Self::Value, E> {
                Ok(DmapValue::I8(value))
            }

            fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E> {
                Ok(DmapValue::U8(value))
            }

            fn visit_i16<E>(self, value: i16) -> Result<Self::Value, E> {
                Ok(DmapValue::I16(value))
            }

            fn visit_u16<E>(self, value: u16) -> Result<Self::Value, E> {
                Ok(DmapValue::U16(value))
            }

            fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E> {
                Ok(DmapValue::I32(value))
            }
            
            fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E> {
                Ok(DmapValue::U32(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(DmapValue::I64(value))
            }
            
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(DmapValue::U64(value))
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
                Ok(DmapValue::String(value))
            }

            fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Self::Value, E> {
                Ok(DmapValue::Unknown(value))
            }

            fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
                where V: de::MapAccess<'de>
            {
                let mut vec = Vec::new();

                while let Some((k, v)) = visitor.next_entry()? {
                    vec.push(DmapItem {
                        name: k,
                        value: v,
                    });
                }

                Ok(DmapValue::Container(vec))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

impl<'k> ser::Serialize for ItemName<'k> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer
    {
        match *self {
            ItemName::Name(n) => serializer.serialize_str(n),
            ItemName::Code(c) => serializer.serialize_bytes(&c),
        }
    }
}

impl<'a, 'k> ser::Serialize for DmapValue<'a, 'k> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer
    {
        match *self {
            DmapValue::I8(x) => serializer.serialize_i8(x),
            DmapValue::U8(x) => serializer.serialize_u8(x),
            DmapValue::I16(x) => serializer.serialize_i16(x),
            DmapValue::U16(x) => serializer.serialize_u16(x),
            DmapValue::I32(x) => serializer.serialize_i32(x),
            DmapValue::U32(x) => serializer.serialize_u32(x),
            DmapValue::I64(x) => serializer.serialize_i64(x),
            DmapValue::U64(x) => serializer.serialize_u64(x),
            DmapValue::String(s) => serializer.serialize_str(s),
            DmapValue::Unknown(b) => serializer.serialize_bytes(b),
            DmapValue::Container(ref c) => {
                let mut map = serializer.serialize_map(Some(c.len()))?;
                for e in c {
                    map.serialize_entry(&e.name, &e.value)?;
                }
                map.end()
            }
        }
    }
}
