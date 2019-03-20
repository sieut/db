use common::{ID};
use storage::{PAGE_SIZE};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct BufKey {
    file_id: ID,
    offset: u64,
}

impl BufKey {
    pub const fn new(file_id: ID, offset: u64) -> BufKey {
        BufKey{ file_id, offset }
    }

    pub fn to_filename(&self) -> String {
        format!("{}.dat", self.file_id)
    }

    pub fn byte_offset(&self) -> u64 {
        self.offset * (PAGE_SIZE as u64)
    }
}
