use super::*;

#[test]
fn machine_fields_and_typed_operations() {
    let blob = unsafe {
        Blob::named(
            Name::Machine {
                index: 0x0807060504030201,
                storage_id: 0x1817161514131211,
            },
            0x060504030201,
            true,
        )
    }
    .unwrap();
    let bytes = blob.handle().into_bytes();
    assert_eq!(&bytes[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(&bytes[8..16], &[17, 18, 19, 20, 21, 22, 23, 24]);
    assert_eq!(&bytes[16..24], &[0; 8]);
    assert_eq!(&bytes[24..], &[1, 2, 3, 4, 5, 6, 6, 0x80]);
    let encoded = blob.into_ref().identification().shallow().handle();
    assert_eq!(encoded.as_bytes()[31], 0x9a);
    assert_eq!(&encoded.as_bytes()[..31], &bytes[..31]);
    assert!(Blob::try_from(encoded).is_err());
    let EncodeView::Shallow(thunk) = Encode::try_from(encoded).unwrap().view() else {
        panic!()
    };
    let ThunkView::Identification(reference) = thunk.view() else {
        panic!()
    };
    assert!(
        reference
            .object_descriptor()
            .handle()
            .same_encoding(&blob.handle())
    );
    let reparsed = unsafe { Handle::parse(*encoded.as_bytes()) }.unwrap();
    assert!(reparsed.same_encoding(&encoded));
}

#[test]
fn tag_byte_grammar_is_exhaustive() {
    for tag in 0..=u8::MAX {
        let mut bytes = [0; 32];
        bytes[31] = tag;
        if tag & 0xe0 != 0 {
            bytes[24] = 32;
            bytes[30] = 2;
        }
        let allowed = matches!(
            tag,
            0x00 | 0x10
                | 0x18
                | 0x19
                | 0x1a
                | 0x80
                | 0x90
                | 0x98
                | 0x99
                | 0x9a
                | 0xa0
                | 0xb0
                | 0xb4
                | 0xb5
                | 0xb6
                | 0xb8
                | 0xb9
                | 0xba
                | 0xbc
                | 0xbd
                | 0xbe
                | 0xe0
                | 0xf0
                | 0xf4
                | 0xf5
                | 0xf6
                | 0xf8
                | 0xf9
                | 0xfa
                | 0xfc
                | 0xfd
                | 0xfe
        );
        assert_eq!(Handle::parse_ref(&bytes).is_ok(), allowed, "tag {tag:02x}");
    }
}

#[test]
fn literal_boundaries_and_borrowed_unaligned_views() {
    let empty = Blob::canonical(&[]).unwrap();
    assert_eq!(empty.handle().into_bytes(), [0; 32]);
    let contents = [0xff; 30];
    let literal = Blob::canonical(&contents).unwrap();
    assert_eq!(literal.literal_bytes(), Some(contents.as_slice()));
    assert!(literal.handle().is_canonical());
    assert!(Blob::literal(&[0; 31]).is_err());
    #[repr(align(32))]
    struct Buffer([u8; 33]);
    let mut buffer = Buffer([0; 33]);
    let unaligned = &mut buffer.0;
    unaligned[1..].copy_from_slice(literal.handle().as_bytes());
    let handle = Handle::parse_ref(unaligned[1..].try_into().unwrap()).unwrap();
    let HandleBytesView::Object(blob) = handle.view() else {
        panic!()
    };
    assert_eq!(blob.name(), Name::Literal(contents.as_slice()));
    let native = unsafe { handle.to_owned() };
    assert_eq!(core::mem::align_of::<Handle>(), HANDLE_SIZE);
    assert_eq!(core::mem::size_of::<Handle>(), HANDLE_SIZE);
    assert_eq!(native.as_bytes(), handle.as_bytes());
    let reparsed = unsafe { Handle::parse_native(native.into_native()) }.unwrap();
    assert!(reparsed.same_encoding(&native));
    let HandleView::Object(object) = native.view() else {
        panic!()
    };
    let ObjectView::Blob(blob) = object.view() else {
        panic!()
    };
    assert_eq!(blob.literal_bytes(), Some(contents.as_slice()));
    assert_eq!(handle.as_bytes().as_ptr(), unaligned[1..].as_ptr());
}

#[test]
fn malformed_names_are_rejected() {
    let mut bytes = [0; 32];
    bytes[30] = 31;
    assert_eq!(Handle::parse_ref(&bytes).unwrap_err(), Error::LiteralLength);
    bytes[30] = 0;
    bytes[29] = 1;
    assert_eq!(
        Handle::parse_ref(&bytes).unwrap_err(),
        Error::LiteralPadding
    );
    bytes = [0; 32];
    bytes[30] = 0x20;
    assert_eq!(Handle::parse_ref(&bytes).unwrap_err(), Error::ReservedBits);
    let base = unsafe {
        Blob::named(
            Name::Machine {
                index: 0,
                storage_id: 42,
            },
            0,
            false,
        )
    }
    .unwrap();
    bytes = base.handle().into_bytes();
    bytes[16] = 1;
    assert_eq!(Handle::parse_ref(&bytes).unwrap_err(), Error::ReservedBits);
    bytes = base.handle().into_bytes();
    bytes[30] = 1;
    assert_eq!(Handle::parse_ref(&bytes).unwrap_err(), Error::InvalidName);
    bytes = base.handle().into_bytes();
    bytes[30] = 2 | 0x08;
    assert_eq!(Handle::parse_ref(&bytes).unwrap_err(), Error::ReservedBits);
    assert_eq!(
        unsafe { Blob::named(Name::Canonical(&[0; 24]), 30, false) }.unwrap_err(),
        Error::ShortCanonicalBlob
    );
    let mut bytes = [0; HANDLE_SIZE];
    bytes[24] = 1;
    bytes[30] = 3;
    bytes[31] = 0x80;
    assert_eq!(
        Handle::parse_ref(&bytes).unwrap().name(),
        Name::Local { address: 0 }
    );
    bytes[8] = 1;
    assert_eq!(Handle::parse_ref(&bytes).unwrap_err(), Error::ReservedBits);
    assert_eq!(
        unsafe {
            Tree::named(
                Name::Machine {
                    index: 0,
                    storage_id: 0,
                },
                31,
                false,
            )
        }
        .unwrap_err(),
        Error::TreeLength
    );
    assert_eq!(
        unsafe {
            Blob::named(
                Name::Machine {
                    index: 0,
                    storage_id: 0,
                },
                MAX_BYTE_LEN + 1,
                false,
            )
        }
        .unwrap_err(),
        Error::LengthOverflow
    );
}

#[test]
fn canonical_payloads_verify_and_reject_noncanonical_children() {
    let empty = Tree::canonical(&[]).unwrap();
    let Name::Canonical(hash) = empty.as_handle().name() else {
        panic!()
    };
    assert_eq!(
        hash,
        &[
            0xaf, 0x13, 0x49, 0xb9, 0xf5, 0xf9, 0xa1, 0xa6, 0xa0, 0x40, 0x4d, 0xea, 0x36, 0xdc,
            0xc9, 0x49, 0x9b, 0xcb, 0x25, 0xc9, 0xad, 0xc1, 0x12, 0xb7
        ]
    );
    let contents = [7; 31];
    let blob = Blob::canonical(&contents).unwrap();
    blob.verify(&contents).unwrap();
    assert!(blob.verify(&[8; 31]).is_err());
    let children = [
        Blob::literal(b"child").unwrap().handle(),
        blob.into_ref().identification().strict().handle(),
    ];
    let tree = Tree::canonical(&children).unwrap();
    assert_eq!(tree.byte_len(), 64);
    assert_eq!(tree.entry_count(), 2);
    assert_eq!(tree.checked_len().unwrap(), 2);
    let mut payload = [0; 64];
    payload[..32].copy_from_slice(children[0].as_bytes());
    payload[32..].copy_from_slice(children[1].as_bytes());
    let Name::Canonical(hash) = tree.as_handle().name() else {
        panic!()
    };
    assert_eq!(hash.as_slice(), &blake3::hash(&payload).as_bytes()[..24]);
    tree.verify(&children).unwrap();
    tree.tag_descriptor().verify(&children).unwrap();
    assert!(tree.verify(&[children[1], children[0]]).is_err());
    let machine = unsafe {
        Blob::named(
            Name::Machine {
                index: 0,
                storage_id: 1,
            },
            31,
            false,
        )
    }
    .unwrap();
    assert_eq!(
        Tree::canonical(&[machine.handle()]).unwrap_err(),
        Error::NonCanonicalChild
    );
}

#[test]
fn tree_thunks_preserve_name_metadata_and_tag() {
    let tree = unsafe {
        Tree::named(
            Name::Machine {
                index: 2,
                storage_id: 9,
            },
            64,
            true,
        )
    }
    .unwrap()
    .tag_descriptor();
    for thunk in [tree.into_ref().application(), tree.into_ref().selection()] {
        for encode in [thunk.strict(), thunk.shallow()] {
            let handle = encode.handle();
            assert_eq!(&handle.as_bytes()[..31], &tree.handle().as_bytes()[..31]);
            unsafe { Handle::parse(*handle.as_bytes()) }.unwrap();
            assert_eq!(handle.eq_bit(), Some(true));
            assert_eq!(handle.checked_len().unwrap(), 2);
        }
    }
}

#[test]
fn equality_metadata_and_verification() {
    let literal = Blob::literal(b"value").unwrap();
    let blob = Blob::canonical(&[7; 64]).unwrap();
    for value in [
        literal.handle(),
        literal.into_ref().handle(),
        blob.handle(),
        blob.into_ref().handle(),
    ] {
        assert!(value.is_eq());
    }
    let tree = Tree::canonical(&[literal.into(), blob.into_ref().into()]).unwrap();
    assert!(tree.as_handle().is_eq());
    let thunk = literal.into_ref().identification();
    for value in [
        thunk.handle(),
        thunk.strict().handle(),
        thunk.shallow().handle(),
        tree.into_ref().selection().handle(),
    ] {
        assert!(!value.is_eq());
        let non_eq = Tree::canonical(&[value]).unwrap();
        assert!(!non_eq.as_handle().is_eq());
        assert!(!non_eq.into_ref().as_handle().is_eq());
        let mut forged = non_eq.handle().into_bytes();
        forged[30] |= 4;
        let forged = Tree::try_from(unsafe { Handle::parse(forged) }.unwrap()).unwrap();
        assert_eq!(forged.verify(&[value]), Err(Error::InvalidEq));
    }
    let mut forged = blob.handle().into_bytes();
    forged[30] &= !4;
    let forged = Blob::try_from(unsafe { Handle::parse(forged) }.unwrap()).unwrap();
    assert_eq!(forged.verify(&[7; 64]), Err(Error::InvalidEq));
}
