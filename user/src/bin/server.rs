#![no_std]
#![no_main]

use user::prelude::*;

extern crate user;

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let get = os::atom("get");
    let cas = os::atom("compare-and-swap");
    let recv = os::atom("recv");
    let send = os::atom("send");
    let close = os::atom("close");

    let _: Ref<Blob> = os::call_with_current_continuation(recv).try_into().unwrap();

    let header =
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n";

    let prefix = r#"
<!doctype html>
<html>
<head>
    <title>Hello from Arca!</title>
    <meta charset="utf-8"/>
</head>
<body>
    <h1>Hello from the Arca kernel!</h1>
    <p>You are user #"#.trim();
    let key = os::blob("count".as_bytes());
    let value = loop {
        let old: Ref<Word> = os::call_with_current_continuation(get.clone().apply(key.clone())).try_into().unwrap();
        let value = old.read() + 1;
        let new = os::word(value);
        let result = os::call_with_current_continuation(cas.clone().apply(key.clone()).apply(old.clone()).apply(new.clone()));
        if result.datatype() != DataType::Error {
            break value;
        }
    };
    let mut buf = [0u8; 32];
    let body = numtoa::numtoa_u64(value, 10, &mut buf);
    let body = os::blob(body);
    let suffix = r#"
    !</p>
</body>
</html>
        "#
        .trim();

    os::call_with_current_continuation(send.clone().apply(os::blob(header.as_bytes())));
    os::call_with_current_continuation(send.clone().apply(os::blob(prefix.as_bytes())));
    os::call_with_current_continuation(send.clone().apply(body));
    os::call_with_current_continuation(send.clone().apply(os::blob(suffix.as_bytes())));
    os::call_with_current_continuation(close);

    unreachable!();
}
