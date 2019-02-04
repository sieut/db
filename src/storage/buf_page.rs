use std::io::Cursor;
use std::iter::Iterator;
use byteorder::ByteOrder;
use byteorder::{LittleEndian, ReadBytesExt};
use storage::{PAGE_SIZE};
use storage::buf_key::BufKey;
use tuple::tuple_ptr::TuplePtr;

static HEADER_SIZE: usize = 8;
const UPPER_PTR_RANGE: std::ops::Range<usize> = (0..4);
const LOWER_PTR_RANGE: std::ops::Range<usize> = (4..8);

// Page layout will be similar to Postgres'
// http://www.interdb.jp/pg/pgsql01.html#_1.3.
pub struct BufPage {
    pub buf: Vec<u8>,
    // Values in page's header
    upper_ptr: PagePtr,
    lower_ptr: PagePtr,
    // BufKey for assertions
    buf_key: BufKey,
}

pub type PagePtr = usize;

impl BufPage {
    pub fn load_from(buffer: &[u8; PAGE_SIZE as usize], buf_key: &BufKey)
            -> Result<BufPage, std::io::Error> {
        let mut reader = Cursor::new(&buffer[0..HEADER_SIZE]);
        let upper_ptr = reader.read_u32::<LittleEndian>()? as usize;
        let lower_ptr = reader.read_u32::<LittleEndian>()? as usize;

        Ok(BufPage {
            buf: buffer.to_vec(),
            upper_ptr,
            lower_ptr,
            buf_key: buf_key.clone(),
        })
    }

    pub fn write_tuple_data(&mut self,
                            tuple_data: &[u8],
                            tuple_ptr: Option<&TuplePtr>)
            -> Result<PagePtr, std::io::Error> {
        let ret_offset;

        let page_ptr: PagePtr = match tuple_ptr {
            Some(ptr) => {
                self.is_valid_tuple_ptr(ptr)?;
                // TODO handle this case
                // TODO this case will also happen if a column is of variable length
                if self.tuple_data_len(ptr)? != tuple_data.len() {
                    panic!("Different sized tuple");
                }

                ret_offset = ptr.buf_offset();

                let mut reader = Cursor::new(
                    &self.buf[BufPage::offset_to_ptr(ptr.buf_offset())
                        ..(BufPage::offset_to_ptr(ptr.buf_offset() + 1))]);
                reader.read_u32::<LittleEndian>()? as usize
            },
            None => {
                // TODO Handle this case
                if self.available_data_space() < tuple_data.len() {
                    panic!("Not enough space in page");
                }

                ret_offset = BufPage::ptr_to_offset(self.lower_ptr);

                let new_ptr = self.upper_ptr - tuple_data.len();
                LittleEndian::write_u32(
                    &mut self.buf[self.lower_ptr
                        ..(self.lower_ptr + 4)],
                    new_ptr as u32);

                self.lower_ptr += 4;
                LittleEndian::write_u32(
                    &mut self.buf[LOWER_PTR_RANGE],
                    self.lower_ptr as u32);

                self.upper_ptr -= tuple_data.len();
                LittleEndian::write_u32(
                    &mut self.buf[UPPER_PTR_RANGE],
                    self.upper_ptr as u32);

                new_ptr
            }
        };

        self.buf[page_ptr..page_ptr + tuple_data.len()]
            .clone_from_slice(tuple_data);

        Ok(ret_offset)
    }

    pub fn get_tuple_data(&self, tuple_ptr: &TuplePtr)
            -> Result<&[u8], std::io::Error> {
        self.is_valid_tuple_ptr(tuple_ptr)?;
        let mut reader = Cursor::new(
            &self.buf[BufPage::offset_to_ptr(tuple_ptr.buf_offset())
            ..BufPage::offset_to_ptr(tuple_ptr.buf_offset() + 1)]);
        let start = reader.read_u32::<LittleEndian>()? as usize;

        let end = if tuple_ptr.buf_offset() > 0 {
            let mut reader = Cursor::new(
                &self.buf[BufPage::offset_to_ptr(tuple_ptr.buf_offset() - 1)
                ..BufPage::offset_to_ptr(tuple_ptr.buf_offset())]);
            reader.read_u32::<LittleEndian>()? as usize
        }
        else {
            PAGE_SIZE
        };

        Ok(&self.buf[start..end])
    }

    pub fn iter(&self) -> Iter {
        Iter {
            buf_page: self,
            tuple_ptr: TuplePtr::new(self.buf_key.clone(), 0),
        }
    }

    pub fn upper_ptr(&self) -> PagePtr {
        self.upper_ptr
    }

    pub fn lower_ptr(&self) -> PagePtr {
        self.lower_ptr
    }

    fn offset_to_ptr(buf_offset: usize) -> PagePtr {
        HEADER_SIZE + buf_offset * 4
    }

    fn ptr_to_offset(ptr: PagePtr) -> usize {
        (ptr - HEADER_SIZE) / 4
    }

    pub fn tuple_count(&self) -> usize {
        (self.lower_ptr - HEADER_SIZE) / 4
    }

    fn is_valid_tuple_ptr(&self, tuple_ptr: &TuplePtr)
            -> Result<(), std::io::Error> {
        if self.buf_key != tuple_ptr.buf_key() {
            Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,
                                    "Invalid buf_key"))
        }
        else if tuple_ptr.buf_offset() >= self.tuple_count() {
            Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Invalid buf_offset {}", tuple_ptr.buf_offset())))
        }
        else {
            Ok(())
        }
    }

    fn tuple_data_len(&self, tuple_ptr: &TuplePtr)
            -> Result<usize, std::io::Error> {
        let tuple_data = self.get_tuple_data(tuple_ptr)?;
        Ok(tuple_data.len())
    }

    fn available_data_space(&self) -> usize {
        // - 4 because we also have to make space for a new ptr
        self.upper_ptr - self.lower_ptr - 4
    }
}

pub struct Iter<'a> {
    buf_page: &'a BufPage,
    tuple_ptr: TuplePtr,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        match self.buf_page.get_tuple_data(&self.tuple_ptr) {
            Ok(data) => {
                self.tuple_ptr.inc_buf_offset();
                Some(data)
            },
            Err(_) => None
        }
    }

    fn count(self) -> usize {
        self.buf_page.tuple_count()
    }
}