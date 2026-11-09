mod directory;
use directory::Directory;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn runfiles() -> PathBuf {
    PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap())
}

fn execute(directory: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(runfiles().join(env!("FIX_CLI")))
        .args(arguments)
        .current_dir(directory)
        .env("FIX_TRUST", directory.join(".fix/trust"))
        .env_remove("FIX_SCHEDULER")
        .env_remove("FIX_COLLECTOR")
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .output()
        .unwrap()
}

fn run(directory: &Path, arguments: &[&str], success: bool) -> Vec<u8> {
    let output = execute(directory, arguments);
    assert_eq!(
        output.status.success(),
        success,
        "{arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap().trim_end().into()
}

fn handle(name: &str) -> String {
    format!("@handle(\"{name}\")")
}

fn label(name: &str) -> String {
    format!("@label(\"{name}\")")
}

fn seed(directory: &Path) {
    run(
        directory,
        &[
            "import",
            runfiles().join(env!("FIX_BOOTSTRAP")).to_str().unwrap(),
        ],
        true,
    );
}

fn add(directory: &Path, name: &str, program: &str) {
    let output = Command::new(runfiles().join(env!("FIX_SEED")))
        .args(["add", "."])
        .arg(format!("{name}={}", runfiles().join(program).display()))
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn claim_result(directory: &Path, coupon: &str) -> String {
    let fields = text(run(directory, &["read-tree", coupon], true));
    let reference = fields.lines().nth(1).unwrap();
    let flags = u8::from_str_radix(&reference[62..], 16).unwrap();
    let claim = format!("{}{:02x}", &reference[..62], flags & 0xe0);
    let claim = text(run(directory, &["read-tree", &claim], true));
    claim.lines().nth(2).unwrap().to_owned()
}

#[test]
fn hints_require_an_admitted_object() {
    let directory = Directory::new("authorization");
    run(&directory, &["init"], true);
    seed(&directory);
    let source = directory.join("known");
    std::fs::create_dir(&source).unwrap();
    run(&source, &["init"], true);
    let contents = "an independently named object with a removable provenance record";
    let object = text(run(&source, &["create-blob", "--string", contents], true));
    run(
        &directory,
        &[
            "todo",
            "add",
            &handle(&object),
            "https://example.invalid/unknown",
        ],
        false,
    );
}

#[test]
fn scheduler_acquires_the_named_blob_and_skips_a_mismatching_body() {
    let directory = Directory::new("acquisition");
    run(&directory, &["init"], true);
    seed(&directory);
    let contents = b"the expected object has exactly these canonical bytes";
    let object = text(run(
        &directory,
        &[
            "create-blob",
            "--string",
            std::str::from_utf8(contents).unwrap(),
            "-l",
            "source",
        ],
        true,
    ));
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for correct in [false, true] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let wrong = vec![b'!'; contents.len()];
            let body = if correct { contents.as_slice() } else { &wrong };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        }
    });
    let wrong = format!("http://{address}/a-wrong");
    let correct = format!("http://{address}/b-correct");
    run(
        &directory,
        &[
            "todo",
            "add",
            &label("source"),
            "ftp://example.invalid/unsupported",
            &wrong,
            &correct,
        ],
        true,
    );
    let record = directory.join(".fix/objects").join(&object);
    let backing = record
        .parent()
        .unwrap()
        .join(std::fs::read_link(&record).unwrap());
    std::fs::remove_file(backing).unwrap();
    run(
        &directory,
        &["create-identification-thunk", "source", "-l", "identify"],
        true,
    );
    run(
        &directory,
        &["create-strict-encode", "identify", "-l", "input"],
        true,
    );
    assert_eq!(
        text(run(&directory, &["-j", "1", "eval", &label("input")], true)),
        object
    );
    assert_eq!(run(&directory, &["read-blob", "source"], true), contents);
    server.join().unwrap();
}

#[test]
fn scheduler_acquires_a_local_file_with_an_encoded_name() {
    let directory = Directory::new("local-acquisition");
    run(&directory, &["init"], true);
    seed(&directory);
    let contents = b"the local file must match this already-known Fix object exactly";
    let source = directory.join("b source #%?.bin");
    std::fs::write(&source, contents).unwrap();
    let object = text(run(
        &directory,
        &["create-blob", source.to_str().unwrap(), "-l", "source"],
        true,
    ));
    let wrong = directory.join("a-mismatch.bin");
    std::fs::write(&wrong, vec![b'!'; contents.len()]).unwrap();
    let uri = |path: &Path| {
        format!(
            "file://{}",
            path.to_str()
                .unwrap()
                .replace('%', "%25")
                .replace(' ', "%20")
                .replace('#', "%23")
                .replace('?', "%3F")
        )
    };
    run(
        &directory,
        &["todo", "add", &label("source"), &uri(&wrong), &uri(&source)],
        true,
    );
    let record = directory.join(".fix/objects").join(&object);
    let backing = record
        .parent()
        .unwrap()
        .join(std::fs::read_link(&record).unwrap());
    std::fs::remove_file(backing).unwrap();
    run(
        &directory,
        &["create-identification-thunk", "source", "-l", "identify"],
        true,
    );
    run(
        &directory,
        &["create-strict-encode", "identify", "-l", "input"],
        true,
    );
    assert_eq!(
        text(run(&directory, &["-j", "1", "eval", &label("input")], true)),
        object
    );
    assert_eq!(run(&directory, &["read-blob", "source"], true), contents);
}

#[test]
fn invocation_local_hints_resolve_backing_without_saving_metadata() {
    let directory = Directory::new("extra-hints");
    run(&directory, &["init"], true);
    seed(&directory);
    let source = directory.join("source.bin");
    let contents = b"a source obtained with an invocation-local Todo hint";
    std::fs::write(&source, contents).unwrap();
    let object = text(run(
        &directory,
        &["create-blob", source.to_str().unwrap()],
        true,
    ));
    let reference = text(run(&directory, &["create-ref", &object], true));
    let record = directory.join(".fix/objects").join(&object);
    std::fs::remove_file(
        record
            .parent()
            .unwrap()
            .join(std::fs::read_link(&record).unwrap()),
    )
    .unwrap();
    let thunk = text(run(
        &directory,
        &["create-identification-thunk", &reference],
        true,
    ));
    let input = text(run(&directory, &["create-strict-encode", &thunk], true));
    let uri = format!("file://{}", source.display());
    let mode = text(run(&directory, &["create-blob", "--string", "eval"], true));
    let request = text(run(
        &directory,
        &["create-tree", &mode, "scheduler", "collector", &input],
        true,
    ));
    let coupon = text(run(
        &directory,
        &[
            "-j",
            "2",
            "schedule",
            "--trusted-tagger",
            "collector",
            "--todo",
            &reference,
            &uri,
            &request,
        ],
        true,
    ));
    assert_eq!(claim_result(&directory, &coupon), object);
    assert_eq!(run(&directory, &["read-blob", &object], true), contents);
    let hints = text(run(
        &directory,
        &["todo", "list", &handle(&reference)],
        true,
    ));
    assert!(hints.is_empty());
}

#[test]
fn named_references_do_not_grant_object_authority() {
    let directory = Directory::new("reference-authority");
    run(&directory, &["init"], true);
    seed(&directory);
    let empty = text(run(&directory, &["create-tree", "-"], true));
    let claim = text(run(
        &directory,
        &["create-blob", "--string", "Runnable"],
        true,
    ));
    let tree = text(run(
        &directory,
        &["create-tree", &empty, &empty, &claim],
        true,
    ));
    let flags = u8::from_str_radix(&tree[62..], 16).unwrap();
    let tag = format!("{}{:02x}", &tree[..62], flags | 0x40);
    let reference = format!("{}{:02x}", &tree[..62], flags | 0x50);
    std::fs::write(directory.join("raw.fix"), format!("@handle(\"{tag}\")")).unwrap();
    run(&directory, &["parse", "raw.fix"], false);
    std::fs::write(
        directory.join("pinned.fix"),
        format!("@todo(\"{reference}\" \"unsupported:source\")"),
    )
    .unwrap();
    assert_eq!(
        text(run(&directory, &["parse", "pinned.fix"], true)),
        reference
    );
    run(&directory, &["resolve", &tag], false);
    run(&directory, &["read-tree", &tag], false);
    run(&directory, &["parse", "raw.fix"], false);
    assert_eq!(
        run(&directory, &["read-tree", &tree], true)
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .count(),
        3
    );
}

#[test]
fn pinned_script_imports_remain_nonresident_until_evaluation() {
    let directory = Directory::new("pinned-script");
    run(&directory, &["init"], true);
    seed(&directory);
    let path = directory.join("source.bin");
    let contents = b"pinned input whose name is supplied entirely by the script";
    std::fs::write(&path, contents).unwrap();
    let object = text(run(
        &directory,
        &["create-blob", path.to_str().unwrap()],
        true,
    ));
    let reference = text(run(&directory, &["create-ref", &object], true));
    let record = directory.join(".fix/objects").join(&object);
    std::fs::remove_file(
        record
            .parent()
            .unwrap()
            .join(std::fs::read_link(&record).unwrap()),
    )
    .unwrap();
    std::fs::remove_file(&record).unwrap();
    let uri = format!("file://{}", path.display());
    let recipe = directory.join("recipes");
    std::fs::create_dir(&recipe).unwrap();
    std::fs::write(
        recipe.join("source.fix"),
        format!("@todo(\"{reference}\" \"../source.bin\")\n"),
    )
    .unwrap();
    assert_eq!(
        text(run(&directory, &["parse", "recipes/source.fix"], true)),
        reference
    );
    run(&directory, &["read-blob", &object], false);
    assert_eq!(
        text(run(
            &directory,
            &["todo", "list", &handle(&reference)],
            true
        )),
        uri
    );
    run(
        &directory,
        &["export", &handle(&reference), "source.tar"],
        true,
    );
    let imported = directory.join("imported");
    std::fs::create_dir(&imported).unwrap();
    run(&imported, &["init"], true);
    run(&imported, &["import", "../source.tar"], true);
    seed(&imported);
    assert_eq!(
        text(run(&imported, &["resolve", &reference], true)),
        reference
    );
    assert_eq!(
        text(run(&imported, &["todo", "list", &handle(&reference)], true)),
        uri
    );
    run(&imported, &["resolve", &object], false);
    std::fs::write(
        recipe.join("source.fix"),
        format!("*'@todo(\"{reference}\" \"file:../source.bin\")\n"),
    )
    .unwrap();
    assert_eq!(
        text(run(
            &directory,
            &["-j", "2", "run", "recipes/source.fix"],
            true
        )),
        object
    );
    assert_eq!(run(&directory, &["read-blob", &object], true), contents);
    std::fs::write(
        directory.join("invalid.fix"),
        format!("@todo(\"{object}\" \"{uri}\")\n"),
    )
    .unwrap();
    run(&directory, &["parse", "invalid.fix"], false);
}

#[test]
fn scripts_report_the_names_of_unpinned_http_imports() {
    let directory = Directory::new("script-http");
    run(&directory, &["init"], true);
    seed(&directory);
    let contents = "an unpinned source whose canonical name should be reported";
    let object = text(run(
        &directory,
        &["create-blob", "--string", contents],
        true,
    ));
    let reference = text(run(&directory, &["create-ref", &object], true));
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let uri = format!("http://{}/source", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            contents.len(),
            contents
        )
        .unwrap();
    });
    std::fs::write(
        directory.join("source.fix"),
        format!("*'@todo(\"{uri}\")\n"),
    )
    .unwrap();
    let output = execute(
        &directory,
        &["-j", "1", "-l", "source", "run", "source.fix"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(text(output.stdout), object);
    assert!(String::from_utf8_lossy(&output.stderr).contains(&format!("{uri} → {reference}")));
    assert_eq!(text(run(&directory, &["resolve", "source"], true)), object);
    server.join().unwrap();
}

#[test]
fn scripts_import_a_git_directory() {
    let directory = Directory::new("script-git");
    run(&directory, &["init"], true);
    seed(&directory);
    let expected = expected_git_directory(&directory);
    let (address, server) = git_server(1);
    let uri = format!("git://{address}/fixture#main");
    std::fs::write(
        directory.join("source.fix"),
        format!("*'@todo(\"{uri}\")\n"),
    )
    .unwrap();
    assert_eq!(
        text(run(&directory, &["-j", "1", "run", "source.fix"], true)),
        expected
    );
    server.join().unwrap();
}

fn git_server(connections: usize) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        fn read(stream: &mut std::net::TcpStream) -> Option<Vec<u8>> {
            let mut header = [0; 4];
            stream.read_exact(&mut header).unwrap();
            let length = usize::from_str_radix(std::str::from_utf8(&header).unwrap(), 16).unwrap();
            if length == 0 {
                return None;
            }
            assert!(length >= 4);
            let mut bytes = vec![0; length - 4];
            stream.read_exact(&mut bytes).unwrap();
            Some(bytes)
        }
        fn packet(stream: &mut std::net::TcpStream, bytes: &[u8]) {
            write!(stream, "{:04x}", bytes.len() + 4).unwrap();
            stream.write_all(bytes).unwrap();
        }
        let refs = include_str!(env!("FIX_GIT_REFS"));
        let pack = include_bytes!(env!("FIX_GIT_PACK"));
        for _ in 0..connections {
            let (mut stream, _) = listener.accept().unwrap();
            assert!(
                read(&mut stream)
                    .unwrap()
                    .starts_with(b"git-upload-pack /fixture\0host=")
            );
            for (index, line) in refs
                .lines()
                .filter(|line| !line.starts_with("blob "))
                .enumerate()
            {
                let (name, oid) = line.split_once(' ').unwrap();
                let capabilities = if index == 0 {
                    "\0ofs-delta no-progress side-band-64k shallow"
                } else {
                    ""
                };
                packet(
                    &mut stream,
                    format!("{oid} {name}{capabilities}\n").as_bytes(),
                );
            }
            stream.write_all(b"0000").unwrap();
            let wanted = read(&mut stream).unwrap();
            assert!(wanted.starts_with(b"want "));
            assert_eq!(read(&mut stream).unwrap(), b"deepen 1\n");
            assert!(read(&mut stream).is_none());
            assert_eq!(read(&mut stream).unwrap(), b"done\n");
            let head = refs.lines().next().unwrap().split_once(' ').unwrap().1;
            packet(&mut stream, format!("shallow {head}\n").as_bytes());
            stream.write_all(b"0000").unwrap();
            packet(&mut stream, b"NAK\n");
            for bytes in pack.chunks(137) {
                let mut band = vec![1];
                band.extend_from_slice(bytes);
                packet(&mut stream, &band);
            }
            stream.write_all(b"0000").unwrap();
        }
    });
    (address, server)
}

fn hydrate(directory: &Path, object: &str, uris: &[&str]) {
    let object_expression = handle(object);
    let mut arguments = vec!["todo", "add", object_expression.as_str()];
    arguments.extend_from_slice(uris);
    run(directory, &arguments, true);
    let record = directory.join(".fix/objects").join(object);
    let backing = record
        .parent()
        .unwrap()
        .join(std::fs::read_link(&record).unwrap());
    std::fs::remove_file(backing).unwrap();
    run(
        directory,
        &["create-identification-thunk", object, "-l", "identify"],
        true,
    );
    run(
        directory,
        &["create-strict-encode", "identify", "-l", "input"],
        true,
    );
    assert_eq!(
        text(run(directory, &["-j", "2", "eval", &label("input")], true)),
        object
    );
}

#[test]
fn scheduler_acquires_a_git_blob_and_checks_its_name() {
    let directory = Directory::new("git-blob");
    run(&directory, &["init"], true);
    seed(&directory);
    let payload = include_bytes!(env!("FIX_GIT_PAYLOAD"));
    let source = directory.join("payload");
    std::fs::write(&source, payload).unwrap();
    let object = text(run(
        &directory,
        &["create-blob", source.to_str().unwrap()],
        true,
    ));
    let (address, server) = git_server(2);
    hydrate(
        &directory,
        &object,
        &[
            &format!("git://{address}/fixture#main:file"),
            &format!("git://{address}/fixture#main:payload"),
        ],
    );
    assert_eq!(run(&directory, &["read-blob", &object], true), payload);
    server.join().unwrap();
}

fn expected_git_directory(directory: &Path) -> String {
    let value = |arguments: &[&str]| text(run(directory, arguments, true));
    let zero = value(&["create-blob", "--u64", "0"]);
    let one = value(&["create-blob", "--u64", "1"]);
    let descriptor = |name: &str, kind: &str, size: usize, contents: &str| {
        let name = value(&["create-blob", "--string", name]);
        let kind = value(&["create-blob", "--u8", kind]);
        let size = value(&["create-blob", "--u64", &size.to_string()]);
        let stat = value(&[
            "create-tree",
            &zero,
            &kind,
            &one,
            &size,
            &zero,
            &zero,
            &zero,
        ]);
        value(&["create-tree", &name, &stat, contents])
    };
    let original = value(&["create-blob", "--string", "hello world\n"]);
    let changed = value(&["create-blob", "--string", "hello world!\n"]);
    let payload_path = directory.join("payload");
    let payload_bytes = include_bytes!(env!("FIX_GIT_PAYLOAD"));
    std::fs::write(&payload_path, payload_bytes).unwrap();
    let payload = value(&["create-blob", payload_path.to_str().unwrap()]);
    let empty = value(&["create-tree", "-"]);
    let child = descriptor("child", "4", 12, &original);
    let children = value(&["create-tree", &child]);
    let entries = [
        descriptor("file", "4", 13, &changed),
        descriptor("link", "7", 12, &original),
        descriptor("nested", "3", 0, &children),
        descriptor("nested.c", "4", 12, &original),
        descriptor("payload", "4", payload_bytes.len(), &payload),
        descriptor("submodule", "3", 0, &empty),
    ];
    let mut arguments = vec!["create-tree"];
    arguments.extend(entries.iter().map(String::as_str));
    value(&arguments)
}

#[test]
fn schedulers_share_git_directory_import_and_normalization() {
    let directory = Directory::new("git-tree");
    run(&directory, &["init"], true);
    seed(&directory);
    let object = expected_git_directory(&directory);
    let (address, server) = git_server(2);
    let uri = format!("git://{address}/fixture#v1:");
    hydrate(&directory, &object, &[&uri]);
    add(&directory, "importer", env!("FIX_IMPORTER"));
    let metadata = text(run(&directory, &["create-blob", "--string", ""], true));
    let uri = text(run(&directory, &["create-blob", "--string", &uri], true));
    let request = text(run(
        &directory,
        &["create-tree", &metadata, "importer", &uri],
        true,
    ));
    assert_eq!(
        text(run(&directory, &["-j", "1", "schedule", &request], true)),
        object
    );
    server.join().unwrap();
}

#[test]
fn todo_add_uri_discovers_a_file_name_and_preserves_hints() {
    let directory = Directory::new("import-file");
    run(&directory, &["init"], true);
    seed(&directory);
    let path = directory.join("binary source");
    let contents: Vec<_> = (0..=255).collect();
    std::fs::write(&path, &contents).unwrap();
    let uri = format!("file://{}", path.to_str().unwrap().replace(' ', "%20"));
    let object = text(run(&directory, &["todo", "add", &uri], true));
    assert_eq!(run(&directory, &["read-blob", &object], true), contents);
    assert_eq!(
        text(run(
            &directory,
            &["create-blob", path.to_str().unwrap()],
            true
        )),
        object
    );
    let alternate = "https://example.invalid/alternate";
    run(
        &directory,
        &["todo", "add", &handle(&object), alternate],
        true,
    );
    assert_eq!(text(run(&directory, &["todo", "add", &uri], true)), object);
    assert_eq!(
        text(run(&directory, &["todo", "list", &handle(&object)], true)),
        format!("{uri}\n{alternate}")
    );
}

#[test]
fn directory_contents_agree_across_host_paths_and_file_uris() {
    use std::ffi::OsStr;
    use std::os::unix::{ffi::OsStrExt, fs::symlink};

    let directory = Directory::new("import-directory");
    run(&directory, &["init"], true);
    seed(&directory);
    let source = directory.join("source directory");
    std::fs::create_dir_all(source.join("nested/empty")).unwrap();
    std::fs::create_dir(source.join(".fix")).unwrap();
    std::fs::write(source.join(".fix/ignored"), b"repository data").unwrap();
    std::fs::write(source.join("nested/file #%.bin"), [0, 1, 255]).unwrap();
    let name = OsStr::from_bytes(b"raw-\xff");
    std::fs::write(source.join(name), b"raw name").unwrap();
    symlink(name, source.join("link")).unwrap();
    symlink(&source, directory.join("alias")).unwrap();
    let descriptor = text(run(
        &directory,
        &["flatware-import", source.to_str().unwrap(), "source"],
        true,
    ));
    let fields = text(run(&directory, &["read-tree", &descriptor], true));
    let expected = fields.lines().nth(2).unwrap();
    std::fs::write(directory.join("source.fix"), "@file(\"alias\")\n").unwrap();
    assert_eq!(
        text(run(&directory, &["run", "source.fix"], true)),
        expected
    );
    let uri = format!("file://{}", source.to_str().unwrap().replace(' ', "%20"));
    assert_eq!(
        text(run(&directory, &["todo", "add", &uri], true)),
        expected
    );
    hydrate(&directory, expected, &[&uri]);
    add(&directory, "importer", env!("FIX_IMPORTER"));
    let metadata = text(run(&directory, &["create-blob", "--string", ""], true));
    let uri = text(run(&directory, &["create-blob", "--string", &uri], true));
    let request = text(run(
        &directory,
        &["create-tree", &metadata, "importer", &uri],
        true,
    ));
    for jobs in ["1", "2"] {
        assert_eq!(
            text(run(&directory, &["-j", jobs, "schedule", &request], true)),
            expected
        );
    }
}

#[test]
fn todo_add_uri_infers_git_blobs_and_directories() {
    let directory = Directory::new("import-git");
    run(&directory, &["init"], true);
    seed(&directory);
    let expected_tree = expected_git_directory(&directory);
    let expected_blob = text(run(
        &directory,
        &["create-blob", directory.join("payload").to_str().unwrap()],
        true,
    ));
    let (address, server) = git_server(2);
    for (selection, expected) in [("main:payload", expected_blob), ("v1:", expected_tree)] {
        let uri = format!("git://{address}/fixture#{selection}");
        let object = text(run(&directory, &["todo", "add", &uri], true));
        assert_eq!(object, expected);
        assert_eq!(
            text(run(&directory, &["todo", "list", &handle(&object)], true)),
            uri
        );
    }
    server.join().unwrap();
}

#[test]
fn unavailable_todo_uri_reports_the_source() {
    let directory = Directory::new("import-unavailable");
    run(&directory, &["init"], true);
    seed(&directory);
    let uri = format!("file://{}/missing", directory.display());
    let output = execute(&directory, &["todo", "add", &uri]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains(&uri));
}

#[test]
fn thin_exports_preserve_hints_without_including_resident_source() {
    let directory = Directory::new("thin-resident");
    run(&directory, &["init"], true);
    seed(&directory);
    let contents = "resident source data that should not be included in the recipe archive";
    let object = text(run(
        &directory,
        &["create-blob", "--string", contents],
        true,
    ));
    let reference = text(run(&directory, &["create-ref", &object], true));
    let root = text(run(
        &directory,
        &["create-tree", &reference, "-l", "recipe"],
        true,
    ));
    let uris = ["unsupported://first", "unsupported://second"];
    run(
        &directory,
        &["todo", "add", &handle(&object), uris[0], uris[1], uris[0]],
        true,
    );
    run(
        &directory,
        &[
            "export",
            "--without-backing",
            &handle(&object),
            &label("recipe"),
            "thin.tar",
        ],
        true,
    );
    let imported = directory.join("imported");
    std::fs::create_dir(&imported).unwrap();
    run(&imported, &["init"], true);
    run(&imported, &["import", "../thin.tar"], true);
    seed(&imported);
    assert_eq!(text(run(&imported, &["read-tree", &root], true)), reference);
    let hints = text(run(&imported, &["todo", "list", &handle(&reference)], true));
    assert_eq!(
        hints.lines().collect::<std::collections::BTreeSet<_>>(),
        uris.into_iter().collect()
    );
    assert_eq!(
        text(run(&imported, &["eval", &handle(&reference)], true)),
        reference
    );
    run(&imported, &["read-blob", &object], false);
    assert_eq!(
        text(run(&imported, &["create-blob", "--string", contents], true)),
        object
    );
    assert_eq!(
        text(run(&imported, &["read-blob", &object], true)),
        contents
    );
}
