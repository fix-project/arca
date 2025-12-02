// TODO(kmohr) maybe try to make this pretty later

use ninep::client::file;
use vfs::File;
use alloc::{vec::Vec, boxed::Box, string::String, string::ToString};

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    FileRequest(FileRequest),
    Continuation(Continuation),
    FileResponse(FileResponse),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileRequest {
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Continuation {
    pub continuation_size: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileResponse {
    pub file_size: u32,
    pub file_data: Vec<u8>,
}

#[derive(Debug)]
pub struct ParseError;

pub async fn read_request(mut data_file: Box<dyn File>) -> Result<(Message, Box<dyn File>), ParseError> {
    // Read in the first character
    // R = File Request
    // C = Continuation to resume
    // F = File Response
    let mut first_byte = [0u8; 1];
    data_file.read(&mut first_byte).await.unwrap();
    match first_byte[0] {
        b'R' => {
            log::info!("Received File Request");
            // Assume a filepath won't be more than 256 chars
            let mut file_path = alloc::vec![0u8; 256];
            let file_path_len = data_file.read(&mut file_path).await.unwrap();
            let file_path = core::str::from_utf8(&file_path[..file_path_len]).unwrap().trim();
            log::info!("Requested file path: {}", file_path);

            Ok((Message::FileRequest(FileRequest {
                file_path: file_path.to_string(),
            }), data_file))
        }
        b'C' => {
            log::info!("Received Continuation Request");
            // Read next 4 bytes for continuation size
            let mut size_bytes = [0u8; 4];
            data_file.read(&mut size_bytes).await.unwrap();
            let continuation_size = u32::from_be_bytes(size_bytes);
            log::info!("Continuation size: {}", continuation_size);
            
            let mut data = alloc::vec![0u8; continuation_size as usize];
            let data_len = data_file.read(&mut data).await.unwrap();
            assert!(data_len as u32 == continuation_size, "Read data length does not match continuation size");
            Ok((Message::Continuation(Continuation {
                continuation_size,
                data,
            }), data_file))
        }
        b'F' => {
            log::info!("Received File Response");
            // Read next 4 bytes for file size
            let mut size_bytes = [0u8; 4];
            data_file.read(&mut size_bytes).await.unwrap();
            let file_size = u32::from_be_bytes(size_bytes);
            log::info!("File size: {}", file_size);
            
            // if file_size > 2MB, read in 2MB chunks
            let mut file_data;
            if file_size > 2 * 1024 * 1024 {
                file_data = alloc::vec![0u8; file_size as usize];
                let mut total_read = 0;
                while total_read < file_size as usize {
                    let chunk_size = core::cmp::min(2 * 1024 * 1024, file_size as usize - total_read);
                    let data_len = data_file.read(&mut file_data[total_read..total_read + chunk_size]).await.unwrap();
                    total_read += data_len;
                }
                
            } else {
                file_data = alloc::vec![0u8; file_size as usize];
                let data_len = data_file.read(&mut file_data).await.unwrap();
                if file_size != data_len as u32 {
                    log::error!("Read data length {} does not match file size {}", data_len, file_size);
                    return Err(ParseError);
                }
            }
            Ok((Message::FileResponse(FileResponse {
                file_size,
                file_data,
            }), data_file))
        }
        _ => {
            log::error!("Unknown request type: {}", first_byte[0]);
            Err(ParseError)
        }
    }
}

// use alloc::{string::String, vec::Vec};
// use chumsky::prelude::*;



// fn parse_u32<'a>() -> impl Parser<'a, &'a [u8], u32, extra::Err<Rich<'a, u8>>> {
//     use chumsky::prelude::*;
//     any()
//         .filter(|x: &u8| x.is_ascii_digit())
//         .repeated()
//         .at_least(1)
//         .collect()
//         .map(|x: Vec<u8>| String::from_utf8(x))
//         .unwrapped()
//         .map(|x: String| x.parse())
//         .unwrapped()
//         .labelled("u32")
// }

// fn parse_string<'a>() -> impl Parser<'a, &'a [u8], String, extra::Err<Rich<'a, u8>>> {
//     use chumsky::prelude::*;
//     any()
//         .repeated()
//         .at_least(1)
//         .collect()
//         .map(|x: Vec<u8>| String::from_utf8(x))
//         .unwrapped()
//         .labelled("string")
// }

// fn file_request<'a>() -> impl Parser<'a, &'a [u8], FileRequest, extra::Err<Rich<'a, u8>>> {
//     use chumsky::prelude::*;
//     just(b'R')
//         .ignore_then(
//             any()
//                 .repeated()
//                 .at_least(1)
//                 .collect()
//                 .map(|x: Vec<u8>| String::from_utf8(x))
//                 .unwrapped(),
//         )
//         .map(|file_path| FileRequest { file_path })
//         .labelled("file_request")
// }

// fn continuation_header<'a>() -> impl Parser<'a, &'a [u8], ContinuationHeader, extra::Err<Rich<'a, u8>>> {
//     use chumsky::prelude::*;
//     just(b'C')
//         .ignore_then(parse_u32())
//         .map(|continuation_size| ContinuationHeader { continuation_size })
//         .labelled("continuation_header")
// }

// fn file_response_header<'a>() -> impl Parser<'a, &'a [u8], FileResponseHeader, extra::Err<Rich<'a, u8>>> {
//     use chumsky::prelude::*;
//     just(b'F')
//         .ignore_then(parse_u32())
//         .map(|file_size| FileResponseHeader { file_size })
//         .labelled("file_response_header")
// }

// pub fn request<'a>() -> impl Parser<'a, &'a [u8], Message, extra::Err<Rich<'a, u8>>> {
//     use chumsky::prelude::*;
//     choice((
//         file_request().map(Message::FileRequest),
//         continuation_header().map(Message::ContinuationHeader),
//         file_response_header().map(Message::FileResponseHeader),
//     ))
//     .labelled("request")
// }