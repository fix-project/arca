// runs with command: cargo test -p kernel --target=x86_64-unknown-none
#[cfg(test)]
mod tests {
    extern crate alloc;

    use crate::prelude::*;
    use crate::types::Error;

    // Verifies serialize and deserialize for the null value.
    #[test]
    fn test_serde_null() {
        let null = Value::Null(Null::new());
        let bytes_vec = postcard::to_allocvec(&null).unwrap();
        let deserialized_null: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized_null, null);
    }

    // Verifies serialize and deserialize for a word value.
    #[test]
    fn test_serde_word() {
        let word = Value::Word(1.into());
        let bytes_vec = postcard::to_allocvec(&word).unwrap();
        let deserialized_word: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized_word, word);
    }

    // Verifies serialize and deserialize for a blob value.
    #[test]
    fn test_serde_blob() {
        let blob = Value::Blob("hello, world!".into());
        let bytes_vec = postcard::to_allocvec(&blob).unwrap();
        let deserialized_blob: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized_blob, blob);
    }

    // Verifies serialize and deserialize for a tuple value.
    #[test]
    fn test_serde_tuple() {
        let tuple = Value::Tuple((1, 2, 3).into());
        let bytes_vec = postcard::to_allocvec(&tuple).unwrap();
        let deserialized_tuple: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized_tuple, tuple);
    }

    // Verifies serialize and deserialize for a page value.
    #[test]
    fn test_serde_page() {
        let page = Value::Page(Page::new(1));
        let bytes_vec = postcard::to_allocvec(&page).unwrap();
        let deserialized_page: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized_page, page);
    }

    // Verifies serialize and deserialize for a table value.
    #[test]
    fn test_serde_table() {
        let table = Value::Table(Table::new(1));
        let bytes_vec = postcard::to_allocvec(&table).unwrap();
        let deserialized_table: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized_table, table);
    }

    // Verifies serialize and deserialize for a read-only page entry.
    #[test]
    fn test_serde_ropage() {
        let ropage = Entry::ROPage(Page::new(1));
        let bytes_vec = postcard::to_allocvec(&ropage).unwrap();
        let deserialized_ropage: Entry = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized_ropage, ropage);
    }

    // Verifies serialize and deserialize for a read-write page entry.
    #[test]
    fn test_serde_rwpage() {
        let rwpage = Entry::RWPage(Page::new(1));
        let bytes_vec = postcard::to_allocvec(&rwpage).unwrap();
        let deserialized_rwpage: Entry = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized_rwpage, rwpage);
    }

    // Verifies serialize and deserialize for a read-only table entry.
    #[test]
    fn test_serde_rotable() {
        let rotable = Entry::ROTable(Table::new(1));
        let bytes_vec = postcard::to_allocvec(&rotable).unwrap();
        let deserialized_rotable: Entry = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized_rotable, rotable);
    }

    // Verifies serialize and deserialize for a read-write table entry.
    #[test]
    fn test_serde_rwtable() {
        let rwtable = Entry::RWTable(Table::new(1));
        let bytes_vec = postcard::to_allocvec(&rwtable).unwrap();
        let deserialized_rwtable: Entry = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized_rwtable, rwtable);
    }

    // Ensures unknown Value variants cause the expected serde error.
    #[test]
    fn test_value_error() {
        let unknown_variant = [7, 0];
        let deserialized: Result<Value, postcard::Error> = postcard::from_bytes(&unknown_variant);
        let deserialized_error = deserialized.expect_err("should have been err");
        let error = serde::de::Error::unknown_variant(
            "7",
            &["Null", "Word", "Blob", "Tuple", "Page", "Table"],
        );
        assert_eq!(deserialized_error, error);
    }

    // Ensures unknown Entry variants cause the expected serde error.
    #[test]
    fn test_entry_error() {
        let unknown_variant = [5, 0];
        let deserialized: Result<Entry, postcard::Error> = postcard::from_bytes(&unknown_variant);
        let deserialized_error = deserialized.expect_err("should have been err");
        let error =
            serde::de::Error::unknown_variant("5", &["Null", "ROPage", "RWPage", "ROTable", "RWTable"]);
        assert_eq!(deserialized_error, error);
    }

    // Confirms datatype tagging and default value behavior.
    #[test]
    fn test_value_datatype_and_defaults() {
        let null = Value::Null(Null::new());
        let word = Value::Word(Word::new(42));
        let blob = Value::Blob(Blob::from("hi"));
        let tuple = Value::Tuple(Tuple::from((1u64, "x")));
        let page = Value::Page(Page::new(1));
        let table = Value::Table(Table::new(1));

        assert_eq!(null.datatype(), DataType::Null);
        assert_eq!(word.datatype(), DataType::Word);
        assert_eq!(blob.datatype(), DataType::Blob);
        assert_eq!(tuple.datatype(), DataType::Tuple);
        assert_eq!(page.datatype(), DataType::Page);
        assert_eq!(table.datatype(), DataType::Table);
        assert_eq!(Value::default().datatype(), DataType::Null);
    }

    // Checks word read semantics and byte size.
    #[test]
    fn test_word_read_and_byte_size() {
        let word = Word::new(0xdeadbeef);
        assert_eq!(word.read(), 0xdeadbeef);

        let value = Value::Word(word);
        assert_eq!(value.byte_size(), core::mem::size_of::<u64>());
    }

    // Confirms blob length and read behavior.
    #[test]
    fn test_blob_read_and_len() {
        let blob = Blob::from("hello");
        assert_eq!(blob.len(), 5);

        let mut buf = [0u8; 8];
        let read = blob.read(0, &mut buf);
        assert_eq!(read, 5);
        assert_eq!(&buf[..5], b"hello");
    }

    // Verifies blob reads with an offset return the expected suffix.
    #[test]
    fn test_blob_read_with_offset() {
        let blob = Blob::from("offset");
        let mut buf = [0u8; 8];
        let read = blob.read(3, &mut buf);
        assert_eq!(read, 3);
        assert_eq!(&buf[..3], b"set");
    }

    // Ensures invalid UTF-8 blobs preserve raw bytes on read.
    #[test]
    fn test_blob_invalid_utf8_roundtrip() {
        let bytes = [0xffu8, 0xfeu8, 0xfdu8];
        let blob = Blob::from(bytes.as_slice());
        let mut buf = [0u8; 4];
        let read = blob.read(0, &mut buf);
        assert_eq!(read, bytes.len());
        assert_eq!(&buf[..bytes.len()], &bytes);
    }

    // Validates tuple set/get/take and iteration order.
    #[test]
    fn test_tuple_set_get_take_and_iter() {
        let mut tuple = Tuple::new(3);
        tuple.set(0, 1u64);
        tuple.set(1, "two");
        tuple.set(2, Value::Null(Null::new()));

        assert_eq!(tuple.get(0), Value::Word(Word::new(1)));
        assert_eq!(tuple.get(1), Value::Blob(Blob::from("two")));
        assert_eq!(tuple.get(2), Value::Null(Null::new()));

        let taken = tuple.take(1);
        assert_eq!(taken, Value::Blob(Blob::from("two")));
        assert_eq!(tuple.get(1), Value::Null(Null::new()));

        let items: Vec<Value> = tuple.iter().collect();
        assert_eq!(items.len(), 3);
    }

    // Checks tuple swap semantics and out-of-bounds errors.
    #[test]
    fn test_tuple_swap_and_bounds_errors() {
        let mut tuple = Tuple::new(2);
        tuple.set(0, 1u64);
        tuple.set(1, 2u64);

        let mut replacement = Value::Blob(Blob::from("swap"));
        tuple.swap(0, &mut replacement);
        assert_eq!(replacement, Value::Word(Word::new(1)));
        assert_eq!(tuple.get(0), Value::Blob(Blob::from("swap")));

        let result = <crate::types::Runtime as arca::Runtime>::get_tuple(&tuple, 3);
        assert!(matches!(result, Err(Error::InvalidIndex(3))));
        let result = <crate::types::Runtime as arca::Runtime>::set_tuple(&mut tuple, 3, 5u64.into());
        assert!(matches!(result, Err(Error::InvalidIndex(3))));
    }

    // Confirms tuple byte sizes add up for mixed content.
    #[test]
    fn test_tuple_byte_size() {
        let tuple = Value::Tuple(Tuple::from((1u64, "hi")));
        assert_eq!(tuple.byte_size(), core::mem::size_of::<u64>() + 2);
    }

    // Verifies page read/write behavior and length.
    #[test]
    fn test_page_read_write_len() {
        let mut page = Page::new(1);
        assert_eq!(page.len(), 1 << 12);

        let data = [1u8, 2, 3, 4];
        let written = page.write(0, &data);
        assert_eq!(written, data.len());

        let mut buf = [0u8; 4];
        let read = page.read(0, &mut buf);
        assert_eq!(read, data.len());
        assert_eq!(buf, data);
    }

    // Ensures page read/write with offsets work correctly.
    #[test]
    fn test_page_read_write_with_offset() {
        let mut page = Page::new(1);
        let data = [9u8, 8, 7];
        let written = page.write(4, &data);
        assert_eq!(written, data.len());

        let mut buf = [0u8; 3];
        let read = page.read(4, &mut buf);
        assert_eq!(read, data.len());
        assert_eq!(buf, data);
    }

    // Confirms page size tier selection at thresholds.
    #[test]
    fn test_page_size_tiers() {
        let small = Page::new(1);
        assert_eq!(small.len(), 1 << 12);

        let mid = Page::new((1 << 12) + 1);
        assert_eq!(mid.len(), 1 << 21);

        let large = Page::new((1 << 21) + 1);
        assert_eq!(large.len(), 1 << 30);
    }

    // Verifies default table entry sizes for small and mid tables.
    #[test]
    fn test_table_default_entry_sizes() {
        let table_small = Table::new(1);
        let entry = table_small.get(0).unwrap();
        assert_eq!(entry, Entry::Null(1 << 12));
        assert_eq!(table_small.len(), 1 << 21);

        let table_mid = Table::new((1 << 21) + 1);
        let entry = table_mid.get(0).unwrap();
        assert_eq!(entry, Entry::Null(1 << 21));
        assert_eq!(table_mid.len(), 1 << 30);
    }

    // Ensures tables grow when mapping beyond current range.
    #[test]
    fn test_table_map_growth() {
        let mut table = Table::new(1);
        let entry = Entry::RWPage(Page::new(1));
        let address = 1 << 21;
        let _ = table.map(address, entry).unwrap();
        assert_eq!(table.len(), 1 << 30);
    }

    // Verifies unmap returns None for addresses beyond the table range.
    #[test]
    fn test_table_unmap_out_of_range() {
        let mut table = Table::new(1);
        let missing = table.unmap(table.len() + 1);
        assert!(missing.is_none());
    }

    // Ensures table map and unmap round-trip a page entry.
    #[test]
    fn test_table_map_unmap_roundtrip() {
        let mut table = Table::new(1);
        let entry = Entry::RWPage(Page::new(1));
        let old = table.map(0, entry.clone()).unwrap();
        assert_eq!(old, Entry::Null(1 << 12));

        let unmapped = table.unmap(0);
        assert_eq!(unmapped, Some(entry));
    }

    // Validates symbolic function apply and read behavior.
    #[test]
    fn test_function_symbolic_apply_and_read() {
        let func = Function::symbolic(Word::new(7));
        assert!(func.is_symbolic());

        let func = func.apply(1u64).apply("arg");
        let read_back = func.read_cloned();

        let expected_args = Tuple::from((1u64, "arg"));
        let expected = Value::Tuple(Tuple::from((
            Blob::from("Symbolic"),
            Value::Word(Word::new(7)),
            Value::Tuple(expected_args),
        )));

        assert_eq!(read_back, expected);
    }

    // Ensures function argument order and force semantics.
    #[test]
    fn test_function_apply_order_and_force() {
        let func = Function::symbolic(Word::new(9));
        let func = func.apply(1u64).apply(2u64).apply("three");
        let read_back = func.read_cloned();

        let expected_args = Tuple::from((1u64, 2u64, "three"));
        let expected = Value::Tuple(Tuple::from((
            Blob::from("Symbolic"),
            Value::Word(Word::new(9)),
            Value::Tuple(expected_args),
        )));
        assert_eq!(read_back, expected);

        let forced = func.force();
        match forced {
            Value::Function(f) => assert!(f.is_symbolic()),
            _ => panic!("expected a symbolic function value"),
        }
    }

    // Confirms invalid function construction is rejected.
    #[test]
    fn test_function_new_rejects_invalid_value() {
        let invalid = Value::Word(Word::new(1));
        let result = Function::new(invalid);
        assert!(matches!(result, Err(Error::InvalidValue)));
    }
}
