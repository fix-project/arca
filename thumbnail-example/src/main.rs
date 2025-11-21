#![no_std]
#![no_main]

extern crate alloc;
extern crate user;

use arca::Word;
use user::io::File;
use user::prelude::*;

static OUTPUT_WIDTH: u32 = 256;

fn get_next_token(data: &[u8], mut index: usize) -> (alloc::vec::Vec<u8>, usize) {
    // skip whitespace
    let is_whitespace = |b: u8| b == b' ' || b == b'\n' || b == b'\r' || b == b'\t';
    while is_whitespace(data[index]) {
        index += 1;
    }

    // skip comments
    while data[index] == b'#' {
        while data[index] != b'\n' {
            index += 1;
        }
        index += 1;
        while is_whitespace(data[index]) {
            index += 1;
        }
    }

    // grab data until next whitespace
    let mut token = alloc::vec::Vec::new();
    while !is_whitespace(data[index]) {
        token.push(data[index]);
        index += 1;
    }
    (token, index)
}

pub fn parse_u32(bytes: &[u8]) -> Result<u32, &'static str> {
    let mut n: u32 = 0;

    for &byte in bytes {
        // Convert ASCII digit character to its numeric value
        let digit = match byte.checked_sub(b'0') {
            Some(d) if d <= 9 => u32::from(d),
            _ => return Err("invalid digit"),
        };

        // Multiply current number by 10 and add the new digit
        // Handle potential overflow
        n = n
            .checked_mul(10)
            .and_then(|val| val.checked_add(digit))
            .ok_or("number too large")?;
    }

    Ok(n)
}

fn thumbnail_ppm6(input_bytes: &[u8]) -> u8 {
    // check the magic number
    let index = 0;
    let (magic_number, index) = get_next_token(input_bytes, index);
    if magic_number != b"P6" {
        panic!("Not a PPM P6 file");
    }

    let (width_str, index) = get_next_token(input_bytes, index);
    let (height_str, index) = get_next_token(input_bytes, index);
    let (maxval_str, index) = get_next_token(input_bytes, index);

    let width: u32 = parse_u32(&width_str).expect("could not read width data");
    let height: u32 = parse_u32(&height_str).expect("could not read height data");
    let maxval: u32 = parse_u32(&maxval_str).expect("could not read maxval data");

    user::error::log(
        alloc::format!(
            "PPM image: width={} height={} maxval={}",
            width,
            height,
            maxval
        )
        .as_bytes(),
    );

    if maxval >= 256 {
        panic!("Only maxval < 256 is supported");
    }

    static CHANNELS: u32 = 3;
    let output_height: u32 = (OUTPUT_WIDTH as f32 / width as f32 * height as f32) as u32;
    let header_data = alloc::format!("P6\n{} {}\n{}\n", OUTPUT_WIDTH, output_height, maxval);
    let out_index = header_data.len();
    let mut thumbnail =
        alloc::vec![0u8; out_index + (OUTPUT_WIDTH * output_height * CHANNELS) as usize];
    thumbnail[..out_index].copy_from_slice(header_data.as_bytes());
    let scale_x = width as f32 / OUTPUT_WIDTH as f32;
    let scale_y = height as f32 / output_height as f32;
    for y in 0..output_height {
        for x in 0..OUTPUT_WIDTH {
            let src_x = (x as f32 * scale_x) as usize;
            let src_y = (y as f32 * scale_y) as usize;
            for c in 0..CHANNELS {
                thumbnail[out_index + ((y * OUTPUT_WIDTH + x) * CHANNELS + c) as usize] =
                    input_bytes[index
                        + (src_y * (width as usize) + src_x) * (CHANNELS as usize)
                        + (c as usize)];
            }
        }
    }

    // write the thumbnail to a new ppm file
    let mut output_file = File::options()
        .write(true)
        .create(true)
        .open("127.0.0.1:11212/data/thumbnail.ppm")
        .expect("could not create output thumbnail file");
    let n = output_file
        .write(&thumbnail)
        .expect("could not write thumbnail data to file");

    user::error::log(alloc::format!("wrote {} bytes to thumbnail.ppm", n).as_bytes());
    output_height as u8
}

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    // TODO(kmohr) take this input file path as argument
    let mut ppm_data = File::options()
        .read(true)
        .open("127.0.0.1:11212/data/falls_1.ppm")
        .expect("could not open ppm file");
    user::error::log("opened ppm file");

    // the sun.ppm file is 12814240 bytes eeeek
    // try allocating 2GB into a Box
    user::error::log("allocating a bunch of space");
    let mut buf = alloc::vec![0; 12814240];
    // let mut buf = alloc::vec![0; 2332861];
    user::error::log("alloced a bunch of space");
    let n = ppm_data
        .read(&mut buf[..])
        .expect("could not read ppm file");

    user::error::log(alloc::format!("read {} bytes from ppm file", n).as_bytes());

    let size = thumbnail_ppm6(&buf);

    crate::io::exit(size);
}
