#![no_std]
#![no_main]

extern crate alloc;

// use alloc::vec::Vec;
// use core::fmt::Write;
// use std::io::Cursor;
// use user::buffer::Buffer;
// use user::io::{self, Buffered, File};

extern crate user;

use user::prelude::*;

// fn get_thumbnail(vec: Vec<u8>, size: u32) -> Vec<u8> {
//     let reader = Cursor::new(vec);
//     let mut thumbnails = thumbnailer::create_thumbnails(
//         reader,
//         mime::IMAGE_PNG,
//         [thumbnailer::ThumbnailSize::Custom((size, size))],
//     )
//     .unwrap();
//
//     let thumbnail = thumbnails.pop().unwrap();
//     let mut buf = Cursor::new(Vec::new());
//     thumbnail.write_png(&mut buf).unwrap();
//
//     buf.into_inner()
// }
//
#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    // let image = std::fs::read("./images/parrot.png").expect("failed to open image");
    // let size = 128;
    // let thumbnail = get_thumbnail(image, size);
    //
    // std::fs::write("thumbnail.png", &thumbnail).expect("Failed to write thumbnail");

    // Assume kernel provides tree of hostname, port, filepath
    // let argument = os::argument();
    // let tree: Tuple = argument.try_into().unwrap();
    // let host_blob: Blob = tree.get(0).try_into().unwrap();
    // let mut buffer = [0u8; 128];
    // let buf_slice: &mut [u8] = &mut buffer;
    // // let hostname = str::from_utf8(host_blob.read(buf_slice, buf_slice));
    //
    // let hostname: Blob = Blob::from("localhost");
    // let port: Word = Word::from(11211);
    // let file_path: Blob =
    //     Blob::from("/home/kmohr/workspace/arca/thumbnail-example/images/parrot.png");
    //
    // // screw it, we hardcoding ... TODO(kmohr)
    // let image_indx: Word = Function::symbolic("get")
    //     // .apply(hostname)
    //     // .apply(port)
    //     .apply(file_path)
    //     .call_with_current_continuation()
    //     .try_into()
    //     .expect("add should return a word");

    let i: Word = Function::symbolic("get")
        .call_with_current_continuation()
        .try_into()
        .expect("should return a word");

    // let state: u64 = 42;
    // let k = os::continuation();

    // let i: Word = Function::symbolic("incr")
    //     .call_with_current_continuation()
    //     .try_into()
    //     .expect("should return a word");

    // if i.read() == 5 {
    //     os::exit(Word::new(i.read() + state));
    // } else {
    //     k.unwrap_or_else(|_| panic!("Something went horribly wrong!"))
    //         .force();
    //     os::exit(Word::new(0));
    // }
    os::exit(i);
}
