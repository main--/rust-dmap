use serde::ser::{self, Serialize, SerializeMap};
use byteorder::{BigEndian, WriteBytesExt, ByteOrder};

use super::Parser;

pub fn to_vec<'a, 'k, T>(parser: &'a Parser<'k>, value: &T) -> Result<Vec<u8>, Error>
    where T: Serialize + ?Sized
{
    let mut serializer = Serializer::new(parser);
    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

pub struct Serializer<'a, 'k: 'a> {
    parser: &'a Parser<'k>,
    output: Vec<u8>,
}

impl<'a, 'k> Serializer<'a, 'k> {
    pub fn new(parser: &'a Parser<'k>) -> Serializer<'a, 'k> {
        Serializer { output: Vec::new(), parser }
    }
}

type Error = ::serde::de::value::Error;

impl<'a, 'k: 'a, 'b> ser::Serializer for &'b mut Serializer<'a, 'k> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = SeqSerializer<'a, 'k, 'b>;
    type SerializeTuple = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = MapSerializer<'a, 'k, 'b>;
    type SerializeStruct = MapSerializer<'a, 'k, 'b>;
    type SerializeStructVariant = ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, _: bool) -> Result<(), Error> { panic!("not supported"); }
    fn serialize_f32(self, _: f32) -> Result<(), Error> { panic!("not supported"); }
    fn serialize_f64(self, _: f64) -> Result<(), Error> { panic!("not supported"); }
    fn serialize_char(self, _: char) -> Result<(), Error> { panic!("not supported"); }
    fn serialize_unit(self) -> Result<(), Error> { panic!("not supported"); }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Error> { panic!("not supported"); }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), Error> { panic!("not supported"); }
    fn serialize_newtype_struct<T: ?Sized>(self, _: &'static str, _: &T) -> Result<(), Error> { panic!("not supported"); }
    fn serialize_newtype_variant<T: ?Sized>(self, _: &'static str, _: u32, _: &'static str, _: &T) -> Result<(), Error> { panic!("not supported"); }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Error> { panic!("not supported"); }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeTupleStruct, Error> { panic!("not supported"); }
    fn serialize_tuple_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeTupleVariant, Error> { panic!("not supported"); }
    fn serialize_struct_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeStructVariant, Error> { panic!("not supported"); }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        let mut buf = [0; 4];
        let offset = self.output.len() - 4;
        buf.copy_from_slice(&mut self.output[offset..]);
        self.output.truncate(offset);
        Ok(SeqSerializer {
            parent: self,
            code: buf,
        })
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Error> {
        if !self.output.is_empty() {
            // write unknown length (MapSerializer will fill in later)
            // (unless output is empty, then this is the root node)
            self.output.write_u32::<BigEndian>(0).unwrap();
        }
        Ok(MapSerializer {
            length_offset: self.output.len(),
            parent: self,
        })
    }

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Error> {
        self.serialize_map(None)
    }


    fn serialize_none(self) -> Result<(), Error> {
        // hack: pretend we never wrote that tag by removing it
        let len = self.output.len() - 4;
        self.output.truncate(len);
        Ok(())
    }

    fn serialize_some<T>(self, t: &T) -> Result<(), Error>
        where T: Serialize + ?Sized
    {
        t.serialize(self)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        self.output.write_u32::<BigEndian>(v.len() as u32).unwrap();
        self.output.extend_from_slice(v);
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<(), Error> {
        self.serialize_bytes(v.as_bytes())
    }

    fn serialize_i8(self, v: i8) -> Result<(), Error> {
        self.serialize_bytes(&[v as u8])
    }

    fn serialize_u8(self, v: u8) -> Result<(), Error> {
        self.serialize_bytes(&[v])
    }

    fn serialize_i16(self, v: i16) -> Result<(), Error> {
        self.output.write_u32::<BigEndian>(2).unwrap();
        self.output.write_i16::<BigEndian>(v).unwrap();
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<(), Error> {
        self.output.write_u32::<BigEndian>(2).unwrap();
        self.output.write_u16::<BigEndian>(v).unwrap();
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<(), Error> {
        self.output.write_u32::<BigEndian>(4).unwrap();
        self.output.write_i32::<BigEndian>(v).unwrap();
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<(), Error> {
        self.output.write_u32::<BigEndian>(4).unwrap();
        self.output.write_u32::<BigEndian>(v).unwrap();
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<(), Error> {
        self.output.write_u32::<BigEndian>(8).unwrap();
        self.output.write_i64::<BigEndian>(v).unwrap();
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<(), Error> {
        self.output.write_u32::<BigEndian>(8).unwrap();
        self.output.write_u64::<BigEndian>(v).unwrap();
        Ok(())
    }
}

pub struct SeqSerializer<'a: 'b, 'k: 'a, 'b> {
    parent: &'b mut Serializer<'a, 'k>,
    code: [u8; 4],
}

impl<'a: 'b, 'k: 'a, 'b> ser::SerializeSeq for SeqSerializer<'a, 'k, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where T: Serialize + ?Sized,
    {
        self.parent.output.extend(&self.code);
        value.serialize(&mut *self.parent)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

pub struct MapSerializer<'a: 'b, 'k: 'a, 'b> {
    parent: &'b mut Serializer<'a, 'k>,
    length_offset: usize,
}

impl<'a: 'b, 'k: 'a, 'b> ser::SerializeMap for MapSerializer<'a, 'k, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where T: Serialize + ?Sized
    {
        let code = match value.serialize(StringExtractor)? {
            Ok(name) => self.parent.parser.types.iter().find(|x| x.name == name).unwrap().code, // fixme unwrap
            Err(c) => c,
        };
        self.parent.output.extend_from_slice(&code);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where T: Serialize + ?Sized
    {
        value.serialize(&mut *self.parent)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.length_offset > 0 {
            let output = &mut self.parent.output;
            let newlen = output.len() as u32;
            let inref = &mut output[self.length_offset-4..];
            BigEndian::write_u32(inref, newlen - self.length_offset as u32);
        }
        Ok(())
    }
}

impl<'a: 'b, 'k: 'a, 'b> ser::SerializeStruct for MapSerializer<'a, 'k, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, name: &'static str, value: &T) -> Result<(), Self::Error>
        where T: Serialize + ?Sized
    {
        self.serialize_key(name)?;
        self.serialize_value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeMap::end(self)
    }
}




struct StringExtractor;

impl ser::Serializer for StringExtractor {
    type Ok = Result<String, [u8; 4]>;
    type Error = Error;
    type SerializeSeq = ser::Impossible<Self::Ok, Error>;
    type SerializeTuple = ser::Impossible<Self::Ok, Error>;
    type SerializeTupleStruct = ser::Impossible<Self::Ok, Error>;
    type SerializeTupleVariant = ser::Impossible<Self::Ok, Error>;
    type SerializeMap = ser::Impossible<Self::Ok, Error>;
    type SerializeStruct = ser::Impossible<Self::Ok, Error>;
    type SerializeStructVariant = ser::Impossible<Self::Ok, Error>;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Ok(value.to_string()))
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        let mut buf = [0; 4];
        buf.copy_from_slice(value);
        Ok(Err(buf))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
        where T: ser::Serialize
    {
        panic!("key not string");
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_unit_struct(self,
                             _name: &'static str)
                             -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_unit_variant(self,
                              _name: &'static str,
                              _variant_index: u32,
                              _variant: &'static str)
                              -> Result<Self::Ok, Self::Error> {
        panic!("key not string");
    }

    fn serialize_newtype_struct<T: ?Sized>(self, _name: &'static str, _value: &T)
                                           -> Result<Self::Ok, Self::Error>
        where T: ser::Serialize,
    {
        panic!("key not string");
    }

    fn serialize_newtype_variant<T: ?Sized>(self,
                                            _name: &'static str,
                                            _variant_index: u32,
                                            _variant: &'static str,
                                            _value: &T)
                                            -> Result<Self::Ok, Self::Error>
        where T: ser::Serialize,
    {
        panic!("key not string");
    }

    fn serialize_seq(self, _len: Option<usize>)
                     -> Result<Self::SerializeSeq, Self::Error> {
        panic!("key not string");
    }

    fn serialize_tuple(self, _len: usize)
                       -> Result<Self::SerializeTuple, Self::Error> {
        panic!("key not string");
    }

    fn serialize_tuple_struct(self, _name: &'static str, _len: usize)
                              -> Result<Self::SerializeTupleStruct, Self::Error> {
        panic!("key not string");
    }

    fn serialize_tuple_variant(self,
                               _name: &'static str,
                               _variant_index: u32,
                               _variant: &'static str,
                               _len: usize)
                               -> Result<Self::SerializeTupleVariant, Self::Error> {
        panic!("key not string");
    }

    fn serialize_map(self, _len: Option<usize>)
                     -> Result<Self::SerializeMap, Self::Error> {
        panic!("key not string");
    }

    fn serialize_struct(self, _name: &'static str, _len: usize)
                        -> Result<Self::SerializeStruct, Self::Error> {
        panic!("key not string");
    }

    fn serialize_struct_variant(self,
                                _name: &'static str,
                                _variant_index: u32,
                                _variant: &'static str,
                                _len: usize)
                                -> Result<Self::SerializeStructVariant, Self::Error> {
        panic!("key not string");
    }
}
