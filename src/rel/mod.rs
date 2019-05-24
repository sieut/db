use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};
use db_state::DbState;
use internal_types::ID;
use log::{LogEntry, OpType};
use meta;
use nom_sql::Literal;
use std::io::Cursor;
use storage::{BufKey, BufMgr, PAGE_SIZE};
use tuple::{TupleDesc, TuplePtr};
use utils;

/// Represent a Relation on disk:
///     - First page of file is metadata of the relation
#[derive(Debug)]
pub struct Rel {
    pub rel_id: ID,
    tuple_desc: TupleDesc,
    num_data_pages: usize,
}

impl Rel {
    pub fn load(
        rel_id: ID,
        db_state: &mut DbState,
    ) -> Result<Rel, std::io::Error> {
        let buf_page =
            db_state.buf_mgr.get_buf(&BufKey::new(rel_id, 0, false))?;
        let lock = buf_page.read().unwrap();

        // The data should have at least num_attr, and an attr type
        assert!(lock.tuple_count() >= 2);

        let mut iter = lock.iter();
        let num_attr = {
            let data = iter.next().unwrap();
            utils::assert_data_len(&data, 4)?;
            let mut cursor = Cursor::new(&data);
            cursor.read_u32::<LittleEndian>()?
        };

        let mut attr_data = vec![];
        for _ in 0..num_attr {
            match iter.next() {
                Some(data) => {
                    attr_data.push(data.to_vec());
                }
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Missing attr types",
                    ));
                }
            };
        }

        let rel_filename = db_state.buf_mgr.key_to_filename(lock.buf_key);
        let num_data_pages =
            utils::file_len(&rel_filename)? as usize / PAGE_SIZE - 1;

        Ok(Rel {
            rel_id,
            tuple_desc: TupleDesc::from_data(&attr_data)?,
            num_data_pages,
        })
    }

    /// Create a new non-SueQL-controlled Relation (table),
    /// must be used when executing CREATE TABLE
    pub fn new<S: Into<String>>(
        name: S,
        tuple_desc: TupleDesc,
        db_state: &mut DbState,
    ) -> Result<Rel, std::io::Error> {
        let rel_id = db_state.meta.get_new_id()?;
        let rel = Rel {
            rel_id,
            tuple_desc,
            num_data_pages: 1,
        };

        Rel::write_new_rel(&mut db_state.buf_mgr, &rel)?;
        // Add an entry to the table info rel
        let table_rel = Rel::load(meta::TABLE_REL_ID, db_state)?;
        let new_entry = table_rel
            .tuple_desc
            .create_tuple_data(vec![name.into(), rel.rel_id.to_string()]);
        table_rel.write_new_tuple(&new_entry, db_state)?;

        Ok(rel)
    }

    pub fn new_temp_rel(
        tuple_desc: TupleDesc,
        db_state: &mut DbState
    ) -> Result<Rel, std::io::Error> {
        let rel_id = db_state.buf_mgr.new_temp_id();
        let rel = Rel {
            rel_id,
            tuple_desc,
            num_data_pages: 1,
        };
        Rel::write_new_rel(&mut db_state.buf_mgr, &rel)?;
        Ok(rel)
    }

    /// Create a SueQL-controlled Relation, for database metadata
    pub fn new_meta_rel(
        rel_id: ID,
        tuple_desc: TupleDesc,
        buf_mgr: &mut BufMgr,
    ) -> Result<Rel, std::io::Error> {
        let rel = Rel {
            rel_id,
            tuple_desc,
            num_data_pages: 1,
        };
        Rel::write_new_rel(buf_mgr, &rel)?;
        Ok(rel)
    }

    pub fn write_new_tuple(
        &self,
        data: &[u8],
        db_state: &mut DbState,
    ) -> Result<TuplePtr, std::io::Error> {
        self.tuple_desc.assert_data_len(data)?;

        let rel_meta = db_state.buf_mgr.get_buf(&self.meta_buf_key())?;
        let _rel_guard = rel_meta.write().unwrap();

        let data_page = db_state.buf_mgr.get_buf(&BufKey::new(
            self.rel_id,
            self.num_data_pages as u64,
            false,
        ))?;
        let mut lock = data_page.write().unwrap();

        if lock.available_data_space() >= data.len() {
            let log_entry = LogEntry::new(
                lock.buf_key,
                OpType::InsertTuple,
                data.to_vec(),
                db_state,
            )?;
            let lsn = log_entry.header.lsn;
            db_state
                .log_mgr
                .write_entries(vec![log_entry], &mut db_state.buf_mgr)?;
            lock.write_tuple_data(data, None, Some(lsn))
        }
        // Not enough space in page, have to create a new one
        else {
            let new_page = db_state.buf_mgr.new_buf(&BufKey::new(
                self.rel_id,
                (self.num_data_pages + 1) as u64,
                false,
            ))?;
            let mut lock = new_page.write().unwrap();

            let log_entry = LogEntry::new(
                lock.buf_key,
                OpType::InsertTuple,
                data.to_vec(),
                db_state,
            )?;
            let lsn = log_entry.header.lsn;
            db_state
                .log_mgr
                .write_entries(vec![log_entry], &mut db_state.buf_mgr)?;
            lock.write_tuple_data(data, None, Some(lsn))
        }
    }

    pub fn scan<Filter, Then>(
        &self,
        db_state: &mut DbState,
        filter: Filter,
        mut then: Then,
    ) -> Result<(), std::io::Error>
    where
        Filter: Fn(&[u8]) -> bool,
        Then: FnMut(&[u8]),
    {
        let rel_meta = db_state.buf_mgr.get_buf(&self.meta_buf_key())?;
        let _rel_guard = rel_meta.read().unwrap();

        for page_idx in 1..self.num_data_pages + 1 {
            let page = db_state.buf_mgr.get_buf(&BufKey::new(
                self.rel_id,
                page_idx as u64,
                false,
            ))?;
            let guard = page.read().unwrap();
            for tup in guard.iter() {
                if filter(&*tup) {
                    then(&*tup);
                }
            }
        }

        Ok(())
    }

    pub fn data_to_strings(
        &self,
        data: &[u8],
        filter_indices: Option<Vec<usize>>,
    ) -> Option<Vec<String>> {
        self.tuple_desc.data_to_strings(data, filter_indices)
    }

    pub fn data_from_literal(&self, inputs: Vec<Vec<Literal>>) -> Vec<Vec<u8>> {
        self.tuple_desc.data_from_literal(inputs)
    }

    pub fn tuple_desc(&self) -> TupleDesc {
        self.tuple_desc.clone()
    }

    fn write_new_rel(
        buf_mgr: &mut BufMgr,
        rel: &Rel,
    ) -> Result<(), std::io::Error> {
        // Create new data file
        let meta_page = buf_mgr.new_buf(&BufKey::new(rel.rel_id, 0, false))?;
        let _first_page =
            buf_mgr.new_buf(&BufKey::new(rel.rel_id, 1, false))?;
        let mut lock = meta_page.write().unwrap();
        // Write num attrs
        {
            let mut data = vec![0u8; 4];
            LittleEndian::write_u32(&mut data, rel.tuple_desc.num_attrs());
            lock.write_tuple_data(&data, None, None)?;
        }
        // Write tuple desc
        {
            let attrs_data = rel.tuple_desc.to_data();
            for tup in attrs_data.iter() {
                lock.write_tuple_data(&tup, None, None)?;
            }
        }
        Ok(())
    }

    fn meta_buf_key(&self) -> BufKey {
        BufKey::new(self.rel_id, 0, false)
    }

    fn to_filename(&self) -> String {
        format!("{}.dat", self.rel_id)
    }
}

#[cfg(test)]
mod tests;
