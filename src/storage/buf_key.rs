use internal_types::ID;
use storage::{Storable, PAGE_SIZE};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct BufKey {
    pub file_id: ID,
    pub offset: u64,
    pub temp: bool,
}

impl BufKey {
    pub const fn new(file_id: ID, offset: u64, temp: bool) -> BufKey {
        BufKey {
            file_id,
            offset,
            temp,
        }
    }

    pub fn to_filename(&self, data_dir: String) -> String {
        if self.temp {
            assert_eq!(self.file_id, 0);
            format!("{}/temp/{}.dat", data_dir, self.file_id)
        } else {
            format!("{}/{}.dat", data_dir, self.file_id)
        }
    }

    pub fn byte_offset(&self) -> u64 {
        self.offset * (PAGE_SIZE as u64)
    }
}

impl Storable for BufKey {
    fn size() -> usize {
        std::mem::size_of::<ID>() + std::mem::size_of::<u64>()
    }

    fn from_data(bytes: Vec<u8>) -> Result<(Self, Vec<u8>), std::io::Error> {
        let (file_id, bytes) = ID::from_data(bytes)?;
        let (offset, bytes) = u64::from_data(bytes)?;
        let key = BufKey::new(file_id, offset, false);
        Ok((key, bytes))
    }

    fn to_data(&self) -> Vec<u8> {
        let mut data = vec![];
        data.append(&mut self.file_id.to_data());
        data.append(&mut self.offset.to_data());
        data
    }
}
