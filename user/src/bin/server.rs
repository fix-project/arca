#![no_std]
#![no_main]

use user::prelude::*;

extern crate user;

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let effect = Function::symbolic("effect");

    let _: Blob = os::call_with_current_continuation(effect.clone()("recv"))
        .try_into()
        .unwrap();

    let header = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n";

    let prefix = r#"
<!doctype html>
<html>
<head>
    <title>Hello from Arca!</title>
    <meta charset="utf-8"/>
</head>
<body>
    <h1>Hello from the Arca kernel!</h1>
    <p>You are user #"#
        .trim();
    let value = loop {
        let old: Word = effect.clone()("get", "count")(Continuation)
            .try_into()
            .unwrap();
        let x = old.read();
        let value = old.read() + 1;
        let result = effect.clone()("compare-and-swap", "count", old, value)(Continuation);
        let result: Word = result.try_into().unwrap();
        if result.read() != x {
            break value;
        }
    };
    let mut buf = [0u8; 32];
    let body = numtoa::numtoa_u64(value, 10, &mut buf);
    let body = Blob::new(body);
    let suffix = r#"
    !</p>
</body>
</html>
        "#
    .trim();

    effect.clone()("send", header, Continuation);
    effect.clone()("send", prefix, Continuation);
    effect.clone()("send", body, Continuation);
    effect.clone()("send", suffix, Continuation);
    effect.clone()("close", Continuation);

    unreachable!();
}
