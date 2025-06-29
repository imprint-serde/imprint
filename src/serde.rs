use std::collections::HashMap;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{
    MAGIC, VERSION,
    error::ImprintError,
    types::{DirectoryEntry, Flags, Header, ImprintRecord, MapKey, SchemaId, TypeCode, Value},
    varint,
};

const HEADER_BYTES: usize = 15;
const DIR_COUNT_BYTES: usize = 5;
const DIR_ENTRY_BYTES: usize = 7;

/// A trait for types that can be written to a byte buffer
pub trait Write {
    /// Write the value to the buffer in the Imprint format
    fn write(&self, buf: &mut BytesMut) -> Result<(), ImprintError>;
}

/// A trait for types that can be read from a byte buffer
pub trait Read: Sized {
    /// Read a value from the buffer, returning the value and number of bytes read
    fn read(bytes: Bytes) -> Result<(Self, usize), ImprintError>;
}

/// A trait for reading values with a known type code
pub trait ValueRead: Sized {
    /// Read a value from the buffer with a known type code, returning the value and number of bytes read
    fn read(type_code: TypeCode, bytes: Bytes) -> Result<(Self, usize), ImprintError>;
}

#[doc = include_str!("../FORMAT.md")]
impl Write for Value {
    fn write(&self, buf: &mut BytesMut) -> Result<(), ImprintError> {
        match self {
            Self::Null => Ok(()),
            Self::Bool(v) => {
                buf.put_u8(if *v { 1 } else { 0 });
                Ok(())
            }
            Self::Int32(v) => {
                buf.put_i32_le(*v);
                Ok(())
            }
            Self::Int64(v) => {
                buf.put_i64_le(*v);
                Ok(())
            }
            Self::Float32(v) => {
                buf.put_f32_le(*v);
                Ok(())
            }
            Self::Float64(v) => {
                buf.put_f64_le(*v);
                Ok(())
            }
            Self::Bytes(v) => {
                varint::encode(v.len() as u32, buf);
                buf.put_slice(v);
                Ok(())
            }
            Self::String(v) => {
                let bytes = v.as_bytes();
                varint::encode(bytes.len() as u32, buf);
                buf.put_slice(bytes);
                Ok(())
            }
            Self::Array(v) => {
                varint::encode(v.len() as u32, buf);
                if v.is_empty() {
                    return Ok(());
                }

                let type_code = v[0].type_code();
                buf.put_u8(type_code as u8);
                for value in v {
                    if value.type_code() != type_code {
                        return Err(ImprintError::SchemaError(format!(
                            "array elements must have same type code: {:?} != {:?}",
                            value.type_code(),
                            type_code
                        )));
                    }
                    value.write(buf)?;
                }
                Ok(())
            }
            Self::Map(m) => {
                varint::encode(m.len() as u32, buf);
                if m.is_empty() {
                    return Ok(());
                }

                let key_type_code = m.keys().next().unwrap().type_code();
                let value_type_code = m.values().next().unwrap().type_code();
                buf.put_u8(key_type_code as u8);
                buf.put_u8(value_type_code as u8);
                for (key, value) in m {
                    if key.type_code() != key_type_code {
                        return Err(ImprintError::SchemaError(format!(
                            "map keys must have same type code: {:?} != {:?}",
                            key.type_code(),
                            key_type_code
                        )));
                    }
                    if value.type_code() != value_type_code {
                        return Err(ImprintError::SchemaError(format!(
                            "map values must have same type code: {:?} != {:?}",
                            value.type_code(),
                            value_type_code
                        )));
                    }
                    key.write(buf)?;
                    value.write(buf)?;
                }
                Ok(())
            }
            Self::Row(v) => v.write(buf),
        }
    }
}

impl Write for MapKey {
    fn write(&self, buf: &mut BytesMut) -> Result<(), ImprintError> {
        match self {
            MapKey::Int32(i) => {
                buf.put_i32_le(*i);
                Ok(())
            }
            MapKey::Int64(i) => {
                buf.put_i64_le(*i);
                Ok(())
            }
            MapKey::Bytes(b) => {
                varint::encode(b.len() as u32, buf);
                buf.put_slice(b);
                Ok(())
            }
            MapKey::String(s) => {
                let bytes = s.as_bytes();
                varint::encode(bytes.len() as u32, buf);
                buf.put_slice(bytes);
                Ok(())
            }
        }
    }
}

impl ValueRead for Value {
    fn read(type_code: TypeCode, mut bytes: Bytes) -> Result<(Self, usize), ImprintError> {
        let mut bytes_read = 0;

        let value = match type_code {
            TypeCode::Null => Value::Null,
            TypeCode::Bool => {
                if !bytes.has_remaining() {
                    return Err(ImprintError::BufferUnderflow {
                        needed: 1,
                        available: 0,
                    });
                }
                let v = bytes.get_u8();
                bytes_read += 1;
                match v {
                    0 => false.into(),
                    1 => true.into(),
                    _ => return Err(ImprintError::SchemaError("invalid boolean value".into())),
                }
            }
            TypeCode::Int32 => {
                if bytes.remaining() < 4 {
                    return Err(ImprintError::BufferUnderflow {
                        needed: 4,
                        available: bytes.remaining(),
                    });
                }
                let v = bytes.get_i32_le();
                bytes_read += 4;
                v.into()
            }
            TypeCode::Int64 => {
                if bytes.remaining() < 8 {
                    return Err(ImprintError::BufferUnderflow {
                        needed: 8,
                        available: bytes.remaining(),
                    });
                }
                let v = bytes.get_i64_le();
                bytes_read += 8;
                v.into()
            }
            TypeCode::Float32 => {
                if bytes.remaining() < 4 {
                    return Err(ImprintError::BufferUnderflow {
                        needed: 4,
                        available: bytes.remaining(),
                    });
                }
                let v = bytes.get_f32_le();
                bytes_read += 4;
                v.into()
            }
            TypeCode::Float64 => {
                if bytes.remaining() < 8 {
                    return Err(ImprintError::BufferUnderflow {
                        needed: 8,
                        available: bytes.remaining(),
                    });
                }
                let v = bytes.get_f64_le();
                bytes_read += 8;
                v.into()
            }
            TypeCode::Bytes => {
                let (len, len_size) = varint::decode(bytes.clone())?;
                bytes.advance(len_size);
                bytes_read += len_size;

                if bytes.remaining() < len as usize {
                    return Err(ImprintError::BufferUnderflow {
                        needed: len as usize,
                        available: bytes.remaining(),
                    });
                }
                let mut v = vec![0; len as usize];
                bytes.copy_to_slice(&mut v);
                bytes_read += len as usize;
                v.into()
            }
            TypeCode::String => {
                let (len, len_size) = varint::decode(bytes.clone())?;
                bytes.advance(len_size);
                bytes_read += len_size;

                if bytes.remaining() < len as usize {
                    return Err(ImprintError::BufferUnderflow {
                        needed: len as usize,
                        available: bytes.remaining(),
                    });
                }
                let mut v = vec![0; len as usize];
                bytes.copy_to_slice(&mut v);
                bytes_read += len as usize;
                let s = String::from_utf8(v).map_err(|_| ImprintError::InvalidUtf8String)?;
                s.into()
            }
            TypeCode::Array => {
                let (len, len_size) = varint::decode(bytes.clone())?;
                bytes.advance(len_size);
                bytes_read += len_size;

                if len == 0 {
                    return Ok((Value::Array(vec![]), bytes_read));
                }

                let element_type = TypeCode::try_from(bytes.get_u8())?;
                bytes_read += 1;

                let mut values = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    let (value, value_size) = Self::read(element_type, bytes.clone())?;
                    bytes.advance(value_size);
                    bytes_read += value_size;
                    values.push(value);
                }
                values.into()
            }
            TypeCode::Map => {
                let (len, len_size) = varint::decode(bytes.clone())?;
                bytes.advance(len_size);
                bytes_read += len_size;

                if len == 0 {
                    return Ok((Value::Map(HashMap::new()), bytes_read));
                }

                let key_type = TypeCode::try_from(bytes.get_u8())?;
                bytes_read += 1;

                let value_type = TypeCode::try_from(bytes.get_u8())?;
                bytes_read += 1;

                let mut map = HashMap::with_capacity(len as usize);
                for _ in 0..len {
                    let (key, key_size) = MapKey::read(key_type, bytes.clone())?;
                    bytes.advance(key_size);
                    bytes_read += key_size;

                    let (value, value_size) = Self::read(value_type, bytes.clone())?;
                    bytes.advance(value_size);
                    bytes_read += value_size;

                    map.insert(key, value);
                }
                map.into()
            }
            TypeCode::Row => {
                let (record, size) = ImprintRecord::read(bytes)?;
                bytes_read += size;
                record.into()
            }
        };
        Ok((value, bytes_read))
    }
}

impl ValueRead for MapKey {
    fn read(type_code: TypeCode, bytes: Bytes) -> Result<(Self, usize), ImprintError> {
        let (value, size) = Value::read(type_code, bytes.clone())?;
        Ok((MapKey::try_from(value)?, size))
    }
}

impl Write for DirectoryEntry {
    fn write(&self, buf: &mut BytesMut) -> Result<(), ImprintError> {
        buf.put_u16_le(self.id);
        buf.put_u8(self.type_code as u8);
        buf.put_u32_le(self.offset);
        Ok(())
    }
}

impl Read for DirectoryEntry {
    fn read(mut bytes: Bytes) -> Result<(Self, usize), ImprintError> {
        if bytes.remaining() < DIR_ENTRY_BYTES {
            return Err(ImprintError::BufferUnderflow {
                needed: DIR_ENTRY_BYTES,
                available: bytes.remaining(),
            });
        }

        let id = bytes.get_u16_le();
        let type_code = TypeCode::try_from(bytes.get_u8())?;
        let offset = bytes.get_u32_le();

        Ok((
            Self {
                id,
                type_code,
                offset,
            },
            DIR_ENTRY_BYTES,
        ))
    }
}

impl Write for SchemaId {
    fn write(&self, buf: &mut BytesMut) -> Result<(), ImprintError> {
        buf.put_u32_le(self.fieldspace_id);
        buf.put_u32_le(self.schema_hash);
        Ok(())
    }
}

impl Read for SchemaId {
    fn read(mut bytes: Bytes) -> Result<(Self, usize), ImprintError> {
        if bytes.remaining() < 8 {
            return Err(ImprintError::BufferUnderflow {
                needed: 8,
                available: bytes.remaining(),
            });
        }

        let fieldspace_id = bytes.get_u32_le();
        let schema_hash = bytes.get_u32_le();

        Ok((
            Self {
                fieldspace_id,
                schema_hash,
            },
            8,
        ))
    }
}

impl Write for Header {
    fn write(&self, buf: &mut BytesMut) -> Result<(), ImprintError> {
        buf.put_u8(MAGIC);
        buf.put_u8(VERSION);
        buf.put_u8(self.flags.0);
        self.schema_id.write(buf)?;
        buf.put_u32_le(self.payload_size);
        Ok(())
    }
}

impl Read for Header {
    fn read(mut bytes: Bytes) -> Result<(Self, usize), ImprintError> {
        if bytes.remaining() < HEADER_BYTES {
            return Err(ImprintError::BufferUnderflow {
                needed: HEADER_BYTES,
                available: bytes.remaining(),
            });
        }

        let magic = bytes.get_u8();
        if magic != MAGIC {
            return Err(ImprintError::InvalidMagic(magic));
        }

        let version = bytes.get_u8();
        if version != VERSION {
            return Err(ImprintError::UnsupportedVersion(version));
        }

        let flags = Flags::new(bytes.get_u8());
        let (schema_id, _) = SchemaId::read(bytes.clone())?;
        bytes.advance(8);

        let payload_size = bytes.get_u32_le();

        Ok((
            Self {
                flags,
                schema_id,
                payload_size,
            },
            HEADER_BYTES,
        ))
    }
}

impl Write for ImprintRecord {
    fn write(&self, buf: &mut BytesMut) -> Result<(), ImprintError> {
        let header_size = HEADER_BYTES;
        let dir_count_size = DIR_COUNT_BYTES;

        let dir_entries_size = self.directory.len() * DIR_ENTRY_BYTES;

        let payload_size = self.payload.len();
        buf.reserve(header_size + dir_count_size + dir_entries_size + payload_size);

        self.header.write(buf)?;

        varint::encode(self.directory.len() as u32, buf);
        for entry in &self.directory {
            entry.write(buf)?;
        }

        buf.put_slice(&self.payload);

        Ok(())
    }
}

impl Read for ImprintRecord {
    fn read(mut bytes: Bytes) -> Result<(Self, usize), ImprintError> {
        let mut bytes_read = 0;

        let (header, header_size) = Header::read(bytes.clone())?;
        bytes.advance(header_size);
        bytes_read += header_size;

        let mut directory = Vec::new();
        let (count, count_size) = varint::decode(bytes.clone())?;
        bytes.advance(count_size);
        bytes_read += count_size;

        for _ in 0..count {
            let (entry, entry_size) = DirectoryEntry::read(bytes.clone())?;
            bytes.advance(entry_size);
            bytes_read += entry_size;
            directory.push(entry);
        }

        let payload = bytes.slice(..header.payload_size as usize);
        bytes.advance(header.payload_size as usize);
        bytes_read += header.payload_size as usize;

        Ok((
            Self {
                header,
                directory,
                payload,
            },
            bytes_read,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::ImprintWriter;
    use proptest::prelude::*;
    use proptest::strategy::{BoxedStrategy, Strategy};
    use proptest::test_runner::TestRunner;

    // Helper function to generate primitive Values
    fn arb_primitive_value() -> BoxedStrategy<Value> {
        prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i32>().prop_map(Value::Int32),
            any::<i64>().prop_map(Value::Int64),
            any::<f32>().prop_map(Value::Float32),
            any::<f64>().prop_map(Value::Float64),
            prop::collection::vec(any::<u8>(), 0..100).prop_map(Value::Bytes),
            ".*".prop_map(Value::String)
        ]
        .boxed()
    }

    // Helper function to generate homogeneous arrays of a specific type
    fn arb_homogeneous_array(element_gen: BoxedStrategy<Value>) -> BoxedStrategy<Value> {
        prop::collection::vec(element_gen, 0..100)
            .prop_map(Value::Array)
            .boxed()
    }

    fn arb_homogeneous_map(
        key_gen: BoxedStrategy<MapKey>,
        value_gen: BoxedStrategy<Value>,
    ) -> BoxedStrategy<Value> {
        prop::collection::hash_map(key_gen, value_gen, 0..10)
            .prop_map(Value::Map)
            .boxed()
    }

    // Helper function to generate a row with a single field
    fn arb_simple_row() -> BoxedStrategy<Value> {
        arb_primitive_value()
            .prop_map(|value| {
                let mut writer = ImprintWriter::new(SchemaId {
                    fieldspace_id: 1,
                    schema_hash: 0xdeadbeef,
                })
                .unwrap();
                writer.add_field(1, value).unwrap();
                Value::Row(Box::new(writer.build().unwrap()))
            })
            .boxed()
    }

    fn arb_value() -> BoxedStrategy<Value> {
        prop_oneof![
            arb_primitive_value(),
            arb_homogeneous_array(any::<i32>().prop_map(Value::Int32).boxed()),
            arb_simple_row()
        ]
        .boxed()
    }

    #[test]
    fn should_handle_error_cases() {
        // Given an invalid magic byte
        let mut buf = BytesMut::new();
        buf.put_u8(0x00); // Wrong magic
        buf.put_u8(VERSION);
        buf.put_u8(0x00);
        buf.put_u64_le(0);
        buf.put_u32_le(0);

        // When trying to read
        // Then it should return an invalid magic error
        assert!(matches!(
            ImprintRecord::read(buf.freeze()),
            Err(ImprintError::InvalidMagic(0x00))
        ));

        // Given an invalid version
        let mut buf = BytesMut::new();
        buf.put_u8(MAGIC);
        buf.put_u8(0xFF); // Wrong version
        buf.put_u8(0x00);
        buf.put_u64_le(0);
        buf.put_u32_le(0);

        // When trying to read
        // Then it should return an unsupported version error
        assert!(matches!(
            ImprintRecord::read(buf.freeze()),
            Err(ImprintError::UnsupportedVersion(0xFF))
        ));

        // Given a truncated buffer
        let mut buf = BytesMut::new();
        buf.put_u8(MAGIC);
        buf.put_u8(VERSION);

        // When trying to read
        // Then it should return a buffer underflow error
        assert!(matches!(
            ImprintRecord::read(buf.freeze()),
            Err(ImprintError::BufferUnderflow { .. })
        ));
    }

    #[test]
    fn test_roundtrip_nested_record() {
        // Create an inner record with an int32 and string field
        let mut inner_writer = ImprintWriter::new(SchemaId {
            fieldspace_id: 2,
            schema_hash: 0xcafebabe,
        })
        .unwrap();
        inner_writer.add_field(1, 42.into()).unwrap();
        inner_writer.add_field(2, "nested".into()).unwrap();
        let inner_record = inner_writer.build().unwrap();

        // Create an outer record containing the inner record and an int64
        let mut outer_writer = ImprintWriter::new(SchemaId {
            fieldspace_id: 1,
            schema_hash: 0xdeadbeef,
        })
        .unwrap();
        outer_writer.add_field(1, inner_record.into()).unwrap();
        outer_writer.add_field(2, 123i64.into()).unwrap();
        let outer_record = outer_writer.build().unwrap();

        // When we serialize and deserialize the outer record
        let mut buf = BytesMut::new();
        outer_record.write(&mut buf).unwrap();
        let (deserialized_record, _) = ImprintRecord::read(buf.freeze()).unwrap();

        // Then the outer record metadata should be preserved
        assert_eq!(deserialized_record.header.schema_id.fieldspace_id, 1);
        assert_eq!(deserialized_record.header.schema_id.schema_hash, 0xdeadbeef);
        assert_eq!(deserialized_record.header.flags, Flags::new(0));
        assert_eq!(deserialized_record.directory.len(), 2);

        // And the outer record values should match
        let got_row = deserialized_record.get_value(1).unwrap().unwrap();
        let got_int64 = deserialized_record.get_value(2).unwrap().unwrap();
        assert_eq!(got_int64, Value::Int64(123));

        // And the inner record should be preserved
        if let Value::Row(inner) = got_row {
            assert_eq!(inner.header.schema_id.fieldspace_id, 2);
            assert_eq!(inner.header.schema_id.schema_hash, 0xcafebabe);
            assert_eq!(inner.header.flags, Flags::new(0));
            assert_eq!(inner.directory.len(), 2);

            let got_inner_int = inner.get_value(1).unwrap().unwrap();
            let got_inner_str = inner.get_value(2).unwrap().unwrap();

            assert_eq!(got_inner_int, Value::Int32(42));
            assert_eq!(got_inner_str, Value::String("nested".to_string()));
        } else {
            panic!("Expected Row value");
        }
    }

    proptest! {
        #[test]
        fn test_roundtrip_simple_record(
            null in Just(Value::Null),
            boolean in any::<bool>().prop_map(Value::Bool),
            int32 in any::<i32>().prop_map(Value::Int32),
            int64 in any::<i64>().prop_map(Value::Int64),
            float32 in any::<f32>().prop_map(Value::Float32),
            float64 in any::<f64>().prop_map(Value::Float64),
            bytes_val in prop::collection::vec(any::<u8>(), 1..100).prop_map(Value::Bytes),
            string in any::<String>().prop_map(Value::String)
        ) {
            let mut writer = ImprintWriter::new(SchemaId {
                fieldspace_id: 1,
                schema_hash: 0xdeadbeef,
            }).map_err(|e| TestCaseError::fail(e.to_string()))?;

            // Add all fields
            writer.add_field(1, null.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;
            writer.add_field(2, boolean.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;
            writer.add_field(3, int32.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;
            writer.add_field(4, int64.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;
            writer.add_field(5, float32.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;
            writer.add_field(6, float64.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;
            writer.add_field(7, bytes_val.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;
            writer.add_field(8, string.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;

            // Build and serialize
            let record = writer.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
            let mut buf = BytesMut::new();
            record.write(&mut buf).map_err(|e| TestCaseError::fail(e.to_string()))?;

            // Deserialize and verify
            let (record, _) = ImprintRecord::read(buf.freeze()).map_err(|e| TestCaseError::fail(e.to_string()))?;

            // Verify metadata
            prop_assert_eq!(record.header.schema_id.fieldspace_id, 1);
            prop_assert_eq!(record.header.schema_id.schema_hash, 0xdeadbeef);
            prop_assert_eq!(record.header.flags, Flags::new(0));
            prop_assert_eq!(record.directory.len(), 8);

            // Verify all values are preserved
            let got = record.get_value(1).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, Some(null));

            let got = record.get_value(2).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, Some(boolean));

            let got = record.get_value(3).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, Some(int32));

            let got = record.get_value(4).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, Some(int64));

            let got = record.get_value(5).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, Some(float32));

            let got = record.get_value(6).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, Some(float64));

            let got = record.get_value(7).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, Some(bytes_val));

            let got = record.get_value(8).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, Some(string));

            // Verify non-existent field returns None
            let got = record.get_value(9).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, None);
        }

        #[test]
        fn prop_roundtrip_arrays(base_value in arb_value()) {
            // Create a strategy for arrays of this type
            let array_strategy = match base_value {
                Value::Null => Just(vec![Value::Null; 3].into()).boxed(),
                Value::Bool(_) => arb_homogeneous_array(any::<bool>().prop_map(Value::Bool).boxed()),
                Value::Int32(_) => arb_homogeneous_array(any::<i32>().prop_map(Value::Int32).boxed()),
                Value::Int64(_) => arb_homogeneous_array(any::<i64>().prop_map(Value::Int64).boxed()),
                Value::Float32(_) => arb_homogeneous_array(any::<f32>().prop_map(Value::Float32).boxed()),
                Value::Float64(_) => arb_homogeneous_array(any::<f64>().prop_map(Value::Float64).boxed()),
                Value::Bytes(_) => arb_homogeneous_array(prop::collection::vec(any::<u8>(), 0..100).prop_map(Value::Bytes).boxed()),
                Value::String(_) => arb_homogeneous_array(".*".prop_map(Value::String).boxed()),
                Value::Array(_) => arb_homogeneous_array(prop::collection::vec(any::<i32>().prop_map(Value::Int32), 0..100).prop_map(Value::Array).boxed()),
                Value::Map(_) => arb_homogeneous_array(prop::collection::hash_map(any::<i32>().prop_map(MapKey::Int32), any::<i32>().prop_map(Value::Int32), 0..100).prop_map(Value::Map).boxed()),
                Value::Row(_) => arb_homogeneous_array(arb_simple_row().boxed()),
            };

            // When generating an array
            let array = array_strategy
                .new_tree(&mut TestRunner::default())
                .map_err(|e| TestCaseError::fail(e.to_string()))?
                .current();

            // Create a record with the array
            let mut writer = ImprintWriter::new(SchemaId {
                fieldspace_id: 1,
                schema_hash: 0xdeadbeef,
            }).map_err(|e| TestCaseError::fail(e.to_string()))?;
            writer.add_field(1, array.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;

            // Build and serialize
            let record = writer.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
            let mut buf = BytesMut::new();
            record.write(&mut buf).map_err(|e| TestCaseError::fail(e.to_string()))?;

            // Deserialize and verify
            let (record, _) = ImprintRecord::read(buf.freeze()).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let got = record.get_value(1).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, Some(array));
        }

        #[test]
        fn prop_roundtrip_maps(
            key_type in prop_oneof![
                Just(TypeCode::Int32),
                Just(TypeCode::Int64),
                Just(TypeCode::Bytes),
                Just(TypeCode::String)
            ],
            base_value in arb_value()
        ) {
            // Create a strategy for keys of the specified type
            let key_strategy = match key_type {
                TypeCode::Int32 => any::<i32>().prop_map(MapKey::Int32).boxed(),
                TypeCode::Int64 => any::<i64>().prop_map(MapKey::Int64).boxed(),
                TypeCode::Bytes => prop::collection::vec(any::<u8>(), 0..100).prop_map(MapKey::Bytes).boxed(),
                TypeCode::String => ".*".prop_map(MapKey::String).boxed(),
                _ => panic!("Unsupported key type"),
            };

            // Create a strategy for values of the specified type
            let value_strategy = match base_value {
                Value::Null => Just(Value::Null).boxed(),
                Value::Bool(_) => any::<bool>().prop_map(Value::Bool).boxed(),
                Value::Int32(_) => any::<i32>().prop_map(Value::Int32).boxed(),
                Value::Int64(_) => any::<i64>().prop_map(Value::Int64).boxed(),
                Value::Float32(_) => any::<f32>().prop_map(Value::Float32).boxed(),
                Value::Float64(_) => any::<f64>().prop_map(Value::Float64).boxed(),
                Value::Bytes(_) => prop::collection::vec(any::<u8>(), 0..100).prop_map(Value::Bytes).boxed(),
                Value::String(_) => ".*".prop_map(Value::String).boxed(),
                Value::Array(_) => arb_homogeneous_array(prop::collection::vec(any::<i32>().prop_map(Value::Int32), 0..100).prop_map(Value::Array).boxed()),
                Value::Map(_) => arb_homogeneous_array(prop::collection::hash_map(any::<i32>().prop_map(MapKey::Int32), any::<i32>().prop_map(Value::Int32), 0..100).prop_map(Value::Map).boxed()),
                Value::Row(_) => arb_homogeneous_array(arb_simple_row().boxed()),
            };

            // Create a strategy for maps with these key and value types
            let map_strategy = arb_homogeneous_map(key_strategy, value_strategy);

            // Generate a map
            let map = map_strategy
                .new_tree(&mut TestRunner::default())
                .map_err(|e| TestCaseError::fail(e.to_string()))?
                .current();

            // Create a record with the map
            let mut writer = ImprintWriter::new(SchemaId {
                fieldspace_id: 1,
                schema_hash: 0xdeadbeef,
            }).map_err(|e| TestCaseError::fail(e.to_string()))?;
            writer.add_field(1, map.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;

            // Build and serialize
            let record = writer.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
            let mut buf = BytesMut::new();
            record.write(&mut buf).map_err(|e| TestCaseError::fail(e.to_string()))?;

            // Deserialize and verify
            let (record, _) = ImprintRecord::read(buf.freeze()).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let got = record.get_value(1).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(got, Some(map));
        }
    }

    #[test]
    fn test_duplicate_field_id() {
        let mut writer = ImprintWriter::new(SchemaId {
            fieldspace_id: 1,
            schema_hash: 0xdeadbeef,
        })
        .unwrap();

        // Add duplicate field IDs
        writer.add_field(1, 42.into()).unwrap();
        writer.add_field(1, 43.into()).unwrap();

        // Build should succeed, last value wins
        let record = writer.build().unwrap();
        assert_eq!(record.directory.len(), 1);
        assert_eq!(record.get_value(1).unwrap(), Some(43.into()));
    }
}
