use super::BinarySerializable;
use std::io;
use std::io::Read;
use std::io::Write;

///   Wrapper over a `u64` that serializes as a variable int.
#[derive(Debug, Eq, PartialEq)]
pub struct VInt(pub u64);

const STOP_BIT: u8 = 128;

pub fn serialize_vint_u32(val: u32) -> (u64, usize) {
    const START_2: u64 = 1 << 7;
    const START_3: u64 = 1 << 14;
    const START_4: u64 = 1 << 21;
    const START_5: u64 = 1 << 28;

    const STOP_1: u64 = START_2 - 1;
    const STOP_2: u64 = START_3 - 1;
    const STOP_3: u64 = START_4 - 1;
    const STOP_4: u64 = START_5 - 1;

    const MASK_1: u64 = 127;
    const MASK_2: u64 = MASK_1 << 7;
    const MASK_3: u64 = MASK_2 << 7;
    const MASK_4: u64 = MASK_3 << 7;
    const MASK_5: u64 = MASK_4 << 7;

    let val = val as u64;
    const STOP_BIT: u64 = 128u64;
    match val {
        0...STOP_1 => (val | STOP_BIT, 1),
        START_2...STOP_2 => (
            (val & MASK_1) | ((val & MASK_2) << 1) | (STOP_BIT << (8)),
            2,
        ),
        START_3...STOP_3 => (
            (val & MASK_1) | ((val & MASK_2) << 1) | ((val & MASK_3) << 2) | (STOP_BIT << (8 * 2)),
            3,
        ),
        START_4...STOP_4 => (
            (val & MASK_1)
                | ((val & MASK_2) << 1)
                | ((val & MASK_3) << 2)
                | ((val & MASK_4) << 3)
                | (STOP_BIT << (8 * 3)),
            4,
        ),
        _ => (
            (val & MASK_1)
                | ((val & MASK_2) << 1)
                | ((val & MASK_3) << 2)
                | ((val & MASK_4) << 3)
                | ((val & MASK_5) << 4)
                | (STOP_BIT << (8 * 4)),
            5,
        ),
    }
}

impl VInt {
    pub fn val(&self) -> u64 {
        self.0
    }

    pub fn deserialize_u64<R: Read>(reader: &mut R) -> io::Result<u64> {
        VInt::deserialize(reader).map(|vint| vint.0)
    }

    pub fn serialize_into_vec(&self, output: &mut Vec<u8>) {
        let mut buffer = [0u8; 10];
        let num_bytes = self.serialize_into(&mut buffer);
        output.extend(&buffer[0..num_bytes]);
    }

    pub fn serialize_into(&self, buffer: &mut [u8; 10]) -> usize {
        let mut remaining = self.0;
        for (i, b) in buffer.iter_mut().enumerate() {
            let next_byte: u8 = (remaining % 128u64) as u8;
            remaining /= 128u64;
            if remaining == 0u64 {
                *b = next_byte | STOP_BIT;
                return i + 1;
            } else {
                *b = next_byte;
            }
        }
        unreachable!();
    }
}

impl BinarySerializable for VInt {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let mut buffer = [0u8; 10];
        let num_bytes = self.serialize_into(&mut buffer);
        writer.write_all(&buffer[0..num_bytes])
    }

    fn deserialize<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut bytes = reader.bytes();
        let mut result = 0u64;
        let mut shift = 0u64;
        loop {
            match bytes.next() {
                Some(Ok(b)) => {
                    result |= u64::from(b % 128u8) << shift;
                    if b >= STOP_BIT {
                        return Ok(VInt(result));
                    }
                    shift += 7;
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Reach end of buffer while reading VInt",
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::serialize_vint_u32;
    use super::VInt;
    use byteorder::{ByteOrder, LittleEndian};
    use common::BinarySerializable;

    fn aux_test_vint(val: u64) {
        let mut v = [14u8; 10];
        let num_bytes = VInt(val).serialize_into(&mut v);
        for i in num_bytes..10 {
            assert_eq!(v[i], 14u8);
        }
        assert!(num_bytes > 0);
        if num_bytes < 10 {
            assert!(1u64 << (7 * num_bytes) > val);
        }
        if num_bytes > 1 {
            assert!(1u64 << (7 * (num_bytes - 1)) <= val);
        }
        let serdeser_val = VInt::deserialize(&mut &v[..]).unwrap();
        assert_eq!(val, serdeser_val.0);
    }

    #[test]
    fn test_vint() {
        aux_test_vint(0);
        aux_test_vint(1);
        aux_test_vint(5);
        aux_test_vint(u64::max_value());
        for i in 1..9 {
            let power_of_128 = 1u64 << (7 * i);
            aux_test_vint(power_of_128 - 1u64);
            aux_test_vint(power_of_128);
            aux_test_vint(power_of_128 + 1u64);
        }
        aux_test_vint(10);
    }

    fn aux_test_serialize_vint_u32(val: u32) {
        let mut buffer = [0u8; 10];
        let mut buffer2 = [0u8; 10];
        let len_vint = VInt(val as u64).serialize_into(&mut buffer);
        let (vint, len) = serialize_vint_u32(val);
        assert_eq!(len, len_vint, "len wrong for val {}", val);
        LittleEndian::write_u64(&mut buffer2, vint);
        assert_eq!(&buffer[..len], &buffer2[..len], "array wrong for {}", val);
    }

    #[test]
    fn test_vint_u32() {
        aux_test_serialize_vint_u32(0);
        aux_test_serialize_vint_u32(1);
        aux_test_serialize_vint_u32(5);
        for i in 1..3 {
            let power_of_128 = 1u32 << (7 * i);
            aux_test_serialize_vint_u32(power_of_128 - 1u32);
            aux_test_serialize_vint_u32(power_of_128);
            aux_test_serialize_vint_u32(power_of_128 + 1u32);
        }
        aux_test_serialize_vint_u32(u32::max_value());
    }
}
