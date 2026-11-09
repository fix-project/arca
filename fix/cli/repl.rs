use fixcli::{Client, expressions::Expressions};
use std::io::{self, BufRead, IsTerminal, Write};

pub fn run(mut client: Client, bindings: Vec<(String, String)>) -> crate::Result<u8> {
    client.reuse_vm();
    let stdin = io::stdin();
    let terminal = stdin.is_terminal();
    let mut input = stdin.lock();
    let base = std::env::current_dir()?;
    loop {
        let mut source = String::new();
        if terminal {
            print!("fix> ");
            io::stdout().flush()?;
        }
        if input.read_line(&mut source)? == 0 || source.trim() == ":quit" {
            break;
        }
        if source.trim() == ":{" {
            source.clear();
            loop {
                if terminal {
                    print!("...> ");
                    io::stdout().flush()?;
                }
                let mut line = String::new();
                if input.read_line(&mut line)? == 0 {
                    return Err("unfinished multiline expression (expected :})".into());
                }
                if line.trim() == ":}" {
                    break;
                }
                source.push_str(&line);
            }
        }
        if source.trim().is_empty() {
            continue;
        }
        client.begin_request();
        let result = (|| -> crate::Result<()> {
            let mut expressions = Expressions::new(&mut client, bindings.clone());
            let parsed = expressions.parse(&source, &base)?;
            let value = expressions.realize(parsed)?;
            println!("{}", client.save(&value)?);
            Ok(())
        })();
        client.end_request()?;
        if let Err(error) = result {
            eprintln!("fix: {error}");
        }
    }
    client.close()?;
    Ok(0)
}
