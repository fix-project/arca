#![no_std]
#![no_main]

extern crate alloc;
extern crate user;

use core::arch::x86_64::{__rdtscp, _rdtsc};

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

fn thumbnail_ppm6(input_bytes: &[u8], former_alloc_time: u64) -> u8 {
    unsafe { core::arch::x86_64::_mm_lfence() };
    let algo_start = unsafe { _rdtsc() };
    unsafe { core::arch::x86_64::_mm_lfence() };

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

    //user::error::log(
    //    alloc::format!(
    //        "PPM image: width={} height={} maxval={}",
    //        width,
    //        height,
    //        maxval
    //    )
    //    .as_bytes(),
    //);

    if maxval >= 256 {
        panic!("Only maxval < 256 is supported");
    }

    static CHANNELS: u32 = 3;
    let output_height: u32 = (OUTPUT_WIDTH as f32 / width as f32 * height as f32) as u32;
    let header_data = alloc::format!("P6\n{} {}\n{}\n", OUTPUT_WIDTH, output_height, maxval);
    let out_index = header_data.len();

    unsafe { core::arch::x86_64::_mm_lfence() };
    let algo_before_alloc = unsafe { _rdtsc() };
    unsafe { core::arch::x86_64::_mm_lfence() };

    let mut thumbnail =
        alloc::vec![0u8; out_index + (OUTPUT_WIDTH * output_height * CHANNELS) as usize];

    unsafe { core::arch::x86_64::_mm_lfence() };
    let after_alloc = unsafe { _rdtsc() };
    unsafe { core::arch::x86_64::_mm_lfence() };

    let dummy = 0;

    thumbnail[..out_index].copy_from_slice(header_data.as_bytes());
    let scale_x = width as f32 / OUTPUT_WIDTH as f32;
    let scale_y = height as f32 / output_height as f32;
    for y in 0..output_height {
        for x in 0..OUTPUT_WIDTH {
            let src_x = (x as f32 * scale_x) as usize;
            let src_y = (y as f32 * scale_y) as usize;
            for c in 0..CHANNELS {
                //core::hint::black_box(dummy);
                thumbnail[out_index + ((y * OUTPUT_WIDTH + x) * CHANNELS + c) as usize] =
                input_bytes[index
                    + (src_y * (width as usize) + src_x) * (CHANNELS as usize)
                    + (c as usize)];
            }
        }
    }

    unsafe { core::arch::x86_64::_mm_lfence() };
    let algo_end = unsafe { _rdtsc() };
    unsafe { core::arch::x86_64::_mm_lfence() };

    user::error::log(
        alloc::format!(
            "TIMING (thumbnail_ppm6):first alloc {}tsc {}tsc(algorithm before alloc:{}tsc alloc:{}tsc algorithm:{}tsc)",
            former_alloc_time,
            algo_end - algo_start,
            algo_before_alloc - algo_start,
            after_alloc - algo_before_alloc,
            algo_end - after_alloc,
        )
        .as_bytes(),
    );

    // write the thumbnail to a new ppm file
    // let mut output_file = File::options()
    //     .write(true)
    //     .create(true)
    //     .open("127.0.0.1:11212/data/thumbnail.ppm")
    //     .expect("could not create output thumbnail file");
    // let n = output_file
    //     .write(&thumbnail)
    //     .expect("could not write thumbnail data to file");
    // user::error::log(alloc::format!("wrote {} bytes to thumbnail.ppm", n).as_bytes());

    output_height as u8
}

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let filename: Blob = os::argument()
        .try_into()
        .expect("could not get filename argument");
    let mut filename_str = [0; 256];
    let filename_size = filename.read(0, &mut filename_str[..]);
    let filename_str = core::str::from_utf8(&filename_str[..filename_size])
        .expect("could not convert filename to str");

    let file_size: Word = os::argument()
        .try_into()
        .expect("could not get file size argument");

    let mut ppm_data = File::options()
        .read(true)
        .open(filename_str)
        .expect("could not open ppm file");
    // XXX: any timestamps taken before this are liable to be wrong due to continuation passing

    unsafe { core::arch::x86_64::_mm_lfence() };
    let before_alloc= unsafe { _rdtsc() };
    unsafe { core::arch::x86_64::_mm_lfence() };

    let mut buf = alloc::vec![0; file_size.read() as usize];

    unsafe { core::arch::x86_64::_mm_lfence() };
    let after_alloc = unsafe { _rdtsc() };
    unsafe { core::arch::x86_64::_mm_lfence() };

    let _n = ppm_data
        .read(&mut buf[..])
        .expect("could not read ppm file");

    let size = thumbnail_ppm6(&buf, after_alloc - before_alloc);

    //user::error::log(
    //    alloc::format!(
    //        "TIMING: \nalloc: {}%\nread file: {}%\nalgorithm: {}%",
    //        (alloc_end - alloc_start) * 100 / total_time,
    //        (read_file_end - alloc_end) * 100 / total_time,
    //        (algo_end - read_file_end) * 100 / total_time,
    //    )
    //    .as_bytes(),
    //);
    crate::io::exit(size);
}
