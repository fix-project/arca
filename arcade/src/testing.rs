use kernel::prelude::*;
extern crate alloc;

fn test_serde_null() {
    let null = Value::Null(Null::new());
    let bytes_vec = postcard::to_allocvec(&null).unwrap();
    let deserialized_null: Value = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_null, null);
}

fn test_serde_word() {
    let word = Value::Word(1.into());
    let bytes_vec = postcard::to_allocvec(&word).unwrap();
    let deserialized_word: Value = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_word, word);
}

fn test_serde_blob() {
    let blob = Value::Blob("hello, world!".into());
    let bytes_vec = postcard::to_allocvec(&blob).unwrap();
    let deserialized_blob: Value = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_blob, blob);
}

fn test_serde_tuple() {
    let tuple = Value::Tuple((1, 2, 3).into());
    let bytes_vec = postcard::to_allocvec(&tuple).unwrap();
    let deserialized_tuple: Value = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_tuple, tuple);
}

fn test_serde_page() {
    let page = Value::Page(Page::new(1));
    let bytes_vec = postcard::to_allocvec(&page).unwrap();
    let deserialized_page: Value = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_page, page);
}

fn test_serde_table() {
    let table = Value::Table(Table::new(1));
    let bytes_vec = postcard::to_allocvec(&table).unwrap();
    let deserialized_table: Value = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_table, table);
}

fn test_serde_function() {
    let arca = Arca::new();
    let inner_func: arca::Function<Runtime> = Function::from(arca);
    let func = Value::Function(inner_func);
    let bytes_vec = postcard::to_allocvec(&func).unwrap();
    let deserialized_func: Value = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_func, func);
}

fn test_serde_ropage() {
    let ropage = Entry::ROPage(Page::new(1));
    let bytes_vec = postcard::to_allocvec(&ropage).unwrap();
    let deserialized_ropage: Entry = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_ropage, ropage);
}

fn test_serde_rwpage() {
    let rwpage = Entry::RWPage(Page::new(1));
    let bytes_vec = postcard::to_allocvec(&rwpage).unwrap();
    let deserialized_rwpage: Entry = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_rwpage, rwpage);
}

fn test_serde_rotable() {
    let rotable = Entry::ROTable(Table::new(1));
    let bytes_vec = postcard::to_allocvec(&rotable).unwrap();
    let deserialized_rotable: Entry = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_rotable, rotable);
}

fn test_serde_rwtable() {
    let rwtable = Entry::RWTable(Table::new(1));
    let bytes_vec = postcard::to_allocvec(&rwtable).unwrap();
    let deserialized_rwtable: Entry = postcard::from_bytes(&bytes_vec).unwrap();
    assert_eq!(deserialized_rwtable, rwtable);
}

pub fn test_runner() {
    test_serde_null();
    test_serde_word();
    test_serde_blob();
    test_serde_tuple();
    test_serde_page();
    test_serde_table();
    test_serde_function();
    test_serde_ropage();
    test_serde_rwpage();
    test_serde_rotable();
    test_serde_rwtable();
}
