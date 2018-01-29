use std::cmp::{Eq,Ordering};
use std::mem::transmute;

pub trait Type {
    type SType;
    type CType;
    const SIZE:usize;

    fn from_bytes(bytes: &[u8]) -> Option<Self::SType>;
    fn to_bytes(&self) -> Option<Vec<u8>>;
    fn get_value(&self) -> Self::CType;
    fn get_size() -> usize { Self::SIZE }
}

#[derive(Copy, Clone)]
pub enum ColumnType {
    Int(Integer),
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct Integer(i32);

impl Integer {
    pub fn new(value: i32) -> Integer { Integer(value) }
}

impl Type for Integer {
    type SType = Integer;
    type CType = i32;
    const SIZE:usize = 4;

    fn from_bytes(bytes: &[u8]) -> Option<Self::SType> {
        if bytes.len() != Self::SIZE {
            None
        }
        else {
            let int_value:i32 = unsafe { transmute::<[u8; Self::SIZE], i32>([bytes[3], bytes[2], bytes[1], bytes[0]]) };
            Some(Integer(int_value))
        }
    }

    fn to_bytes(&self) -> Option<Vec<u8>> {
        let bytes_arr = unsafe { transmute::<i32, [u8; Self::SIZE]>(self.0) };
        Some(bytes_arr.to_vec())
    }

    fn get_value(&self) -> Self::CType {
        self.0
    }
}

impl Eq for Integer {}

impl Ord for Integer {
    fn cmp(&self, other: &Integer) -> Ordering {
        self.0.cmp(&other.0)
    }
}

mod test {
    use types::{Integer, Type};

    #[test]
    fn test_integer() {
        let buffer = [0, 0, 0, 1];
        let int = Integer::from_bytes(&buffer);
        assert_eq!(int.unwrap(), Integer::new(1));
    }
}
