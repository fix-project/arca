// runs with command: cargo test -p kernel --target=x86_64-unknown-none
#[cfg(test)]
mod tests {
    extern crate alloc;

    use crate::types::internal as ktypes;
    use crate::types::{
        Blob as ArcaBlob, Entry as ArcaEntry, Null as ArcaNull, Table as ArcaTable,
        Tuple as ArcaTuple, Value as ArcaValue, Word as ArcaWord,
    };
    use alloc::vec;

    // Verifies internal word read/write semantics.
    #[test]
    fn test_internal_word_read() {
        let word = ktypes::Word::new(123);
        assert_eq!(word.read(), 123);
    }

    // Ensures internal null construction is consistent.
    #[test]
    fn test_internal_null_default() {
        let null = ktypes::Null::new();
        let default = ktypes::Null::default();
        assert_eq!(null, default);
    }

    // Confirms internal blob mutability converts to raw bytes.
    #[test]
    fn test_internal_blob_mutation() {
        let mut blob = ktypes::Blob::new(b"hello".to_vec());
        assert_eq!(blob.len(), 5);
        blob[0] = b'j';
        let bytes = blob.into_inner();
        assert_eq!(&bytes[..], b"jello");
    }

    // Ensures invalid UTF-8 stays as raw bytes internally.
    #[test]
    fn test_internal_blob_invalid_utf8() {
        let bytes = vec![0xffu8, 0xfeu8, 0xfdu8];
        let blob = ktypes::Blob::new(bytes.clone());
        let out = blob.into_inner();
        assert_eq!(&out[..], &bytes);
    }

    // Validates internal tuple defaults and indexing.
    #[test]
    fn test_internal_tuple_defaults() {
        let tuple = ktypes::Tuple::new_with_len(2);
        assert_eq!(tuple.len(), 2);
        assert!(matches!(tuple[0], ArcaValue::Null(_)));
        assert!(matches!(tuple[1], ArcaValue::Null(_)));
    }

    // Verifies internal tuple construction from iterators.
    #[test]
    fn test_internal_tuple_from_iter() {
        let values = vec![
            ArcaValue::Word(ArcaWord::new(1)),
            ArcaValue::Blob(ArcaBlob::from("x")),
        ];
        let tuple: ktypes::Tuple = values.clone().into_iter().collect();
        assert_eq!(tuple.len(), values.len());
        assert_eq!(tuple[0], values[0]);
        assert_eq!(tuple[1], values[1]);
    }

    // Confirms internal page size tiers and shared content.
    #[test]
    fn test_internal_page_size_and_shared() {
        let mut page = ktypes::Page::new(1);
        assert_eq!(page.size(), 1 << 12);
        page[0] = 7;
        let shared = page.clone().shared();
        assert_eq!(shared[0], 7);

        let mid = ktypes::Page::new((1 << 12) + 1);
        assert_eq!(mid.size(), 1 << 21);
    }

    // Verifies internal table size tiers and set/get behavior.
    #[test]
    fn test_internal_table_get_set() {
        let mut table = ktypes::Table::new(1);
        assert_eq!(table.size(), 1 << 21);

        let entry = ArcaEntry::RWPage(crate::types::Page::new(1));
        let old = table.set(0, entry.clone()).unwrap();
        assert_eq!(old, ArcaEntry::Null(1 << 12));

        let fetched = table.get(0);
        assert_eq!(fetched, entry);

        let large = ktypes::Table::new((1 << 21) + 1);
        assert_eq!(large.size(), 1 << 30);
    }

    // Ensures internal table returns default null entries for empty slots.
    #[test]
    fn test_internal_table_default_entry() {
        let table = ktypes::Table::new(1);
        let entry = table.get(10);
        assert_eq!(entry, ArcaEntry::Null(1 << 12));
    }

    // Ensures internal value conversions work as expected.
    #[test]
    fn test_internal_value_conversions() {
        let word = ktypes::Word::new(99);
        let value: ktypes::Value = word.clone().into();
        let roundtrip = ktypes::Word::try_from(value).unwrap();
        assert_eq!(roundtrip, word);

        let blob = ktypes::Blob::new(b"data".to_vec());
        let value: ktypes::Value = blob.clone().into();
        let roundtrip = ktypes::Blob::try_from(value).unwrap();
        assert_eq!(roundtrip, blob);
    }

    // Verifies mismatched internal value conversions return an error.
    #[test]
    fn test_internal_value_conversion_error() {
        let value: ktypes::Value = ktypes::Word::new(1).into();
        let result = ktypes::Blob::try_from(value);
        assert!(result.is_err());
    }

    // Validates symbolic function parsing and read round-trip.
    #[test]
    fn test_internal_function_symbolic_parse() {
        let args = ArcaTuple::from((1u64, "two"));
        let value = ArcaValue::Tuple(ArcaTuple::from((
            ArcaBlob::from("Symbolic"),
            ArcaValue::Word(ArcaWord::new(5)),
            ArcaValue::Tuple(args),
        )));
        let func = ktypes::Function::new(value.clone()).expect("symbolic parse failed");
        assert!(!func.is_arcane());
        assert_eq!(func.read(), value);
    }

    // Ensures invalid function tags are rejected.
    #[test]
    fn test_internal_function_invalid_tag() {
        let value = ArcaValue::Tuple(ArcaTuple::from((
            ArcaBlob::from("Other"),
            ArcaValue::Null(ArcaNull::new()),
        )));
        let func = ktypes::Function::new(value);
        assert!(func.is_none());
    }

    // Verifies arcane function parsing accepts valid layouts.
    #[test]
    fn test_internal_function_arcane_parse() {
        let mut registers = ArcaTuple::new(18);
        for i in 0..18 {
            registers.set(i, ArcaValue::Null(ArcaNull::new()));
        }
        let mut data = ArcaTuple::new(4);
        data.set(0, ArcaValue::Tuple(registers));
        data.set(1, ArcaValue::Table(ArcaTable::new(1)));
        data.set(2, ArcaValue::Tuple(ArcaTuple::new(0)));
        data.set(3, ArcaValue::Tuple(ArcaTuple::new(0)));

        let value = ArcaValue::Tuple(ArcaTuple::from((
            ArcaBlob::from("Arcane"),
            ArcaValue::Tuple(data),
            ArcaValue::Tuple(ArcaTuple::new(0)),
        )));
        let func = ktypes::Function::new(value).expect("arcane parse failed");
        assert!(func.is_arcane());
    }
}
