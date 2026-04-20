// Serialization round-trip tests using postcard.
// Runs with: cargo test -p kernel --target=x86_64-unknown-none

#[cfg(test)]
mod tests {
    extern crate alloc;

    use crate::prelude::*;

    /// Verifies Null serializes and deserializes back to an equal value.
    #[test]
    fn test_serde_null() {
        let null = Value::Null(Null::new());
        let bytes_vec = postcard::to_allocvec(&null).unwrap();
        let deserialized: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized, null);
    }

    /// Verifies Word serializes and deserializes back to an equal value.
    #[test]
    fn test_serde_word() {
        let word = Value::Word(1.into());
        let bytes_vec = postcard::to_allocvec(&word).unwrap();
        let deserialized: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized, word);
    }

    /// Verifies Blob serializes and deserializes back to an equal value.
    #[test]
    fn test_serde_blob() {
        let blob = Value::Blob("hello, world!".into());
        let bytes_vec = postcard::to_allocvec(&blob).unwrap();
        let deserialized: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized, blob);
    }

    /// Verifies Tuple serializes and deserializes back to an equal value.
    #[test]
    fn test_serde_tuple() {
        let tuple = Value::Tuple((1, 2, 3).into());
        let bytes_vec = postcard::to_allocvec(&tuple).unwrap();
        let deserialized: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized, tuple);
    }

    /// Verifies Page serializes and deserializes back to an equal value.
    #[test]
    fn test_serde_page() {
        let page = Value::Page(Page::new(1));
        let bytes_vec = postcard::to_allocvec(&page).unwrap();
        let deserialized: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized, page);
    }

    /// Verifies Table serializes and deserializes back to an equal value.
    #[test]
    fn test_serde_table() {
        let table = Value::Table(Table::new(1));
        let bytes_vec = postcard::to_allocvec(&table).unwrap();
        let deserialized: Value = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized, table);
    }

    /// Verifies a read-only page Entry round-trips through serde.
    #[test]
    fn test_serde_ropage() {
        let ropage = Entry::ROPage(Page::new(1));
        let bytes_vec = postcard::to_allocvec(&ropage).unwrap();
        let deserialized: Entry = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized, ropage);
    }

    /// Verifies a read-write page Entry round-trips through serde.
    #[test]
    fn test_serde_rwpage() {
        let rwpage = Entry::RWPage(Page::new(1));
        let bytes_vec = postcard::to_allocvec(&rwpage).unwrap();
        let deserialized: Entry = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized, rwpage);
    }

    /// Verifies a read-only table Entry round-trips through serde.
    #[test]
    fn test_serde_rotable() {
        let rotable = Entry::ROTable(Table::new(1));
        let bytes_vec = postcard::to_allocvec(&rotable).unwrap();
        let deserialized: Entry = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized, rotable);
    }

    /// Verifies a read-write table Entry round-trips through serde.
    #[test]
    fn test_serde_rwtable() {
        let rwtable = Entry::RWTable(Table::new(1));
        let bytes_vec = postcard::to_allocvec(&rwtable).unwrap();
        let deserialized: Entry = postcard::from_bytes(&bytes_vec).unwrap();
        assert_eq!(deserialized, rwtable);
    }

    /// Ensures deserializing an unknown Value variant produces the expected error.
    #[test]
    fn test_value_unknown_variant_error() {
        let unknown_variant = [7, 0];
        let deserialized: Result<Value, postcard::Error> = postcard::from_bytes(&unknown_variant);
        let deserialized_error = deserialized.expect_err("should have been err");
        let error = serde::de::Error::unknown_variant(
            "7",
            &["Null", "Word", "Blob", "Tuple", "Page", "Table"],
        );
        assert_eq!(deserialized_error, error);
    }

    /// Ensures deserializing an unknown Entry variant produces the expected error.
    #[test]
    fn test_entry_unknown_variant_error() {
        let unknown_variant = [5, 0];
        let deserialized: Result<Entry, postcard::Error> = postcard::from_bytes(&unknown_variant);
        let deserialized_error = deserialized.expect_err("should have been err");
        let error = serde::de::Error::unknown_variant(
            "5",
            &["Null", "ROPage", "RWPage", "ROTable", "RWTable"],
        );
        assert_eq!(deserialized_error, error);
    }
}
