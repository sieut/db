use table::column::Column;

/// Tuple Descriptor struct
///     - columns: vector of Columns which have column's name and data type
///     - paddings: vector of padding bytes of each column to have the fields aligned
///     - tuple_size: size, in bytes, of a row described by this TupleDesc, including padding bytes
pub struct TupleDesc {
    pub columns: Vec<Column>,
    pub paddings: Vec<usize>,
    pub tuple_size: usize,
}

impl TupleDesc {
    pub fn new(columns: &Vec<Column>) -> TupleDesc {
        let mut tuple_size: usize = 0;

        let mut max_fixed: usize = 0;
        for col in columns.iter() {
            if col.column_type.is_fixed_size() && col.column_type.data_size() > max_fixed {
                max_fixed = col.column_type.data_size();
            }
            tuple_size += col.column_type.data_size();
        }

        let mut paddings: Vec<usize> = vec![];
        for col in columns.iter() {
            if col.column_type.data_size() % max_fixed > 0 {
                paddings.push(max_fixed - col.column_type.data_size() % max_fixed);
            }
            else {
                paddings.push(0)
            }

            tuple_size += paddings.last().unwrap();
        }

        TupleDesc {
            columns: (*columns).clone(),
            paddings: paddings,
            tuple_size: tuple_size,
        }
    }
}