use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn run(directory: &Path, arguments: &[&str], input: &[u8], status: i32) -> Vec<u8> {
    let root = PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap());
    let helpers = root.join(env!("FIX_COMMANDS"));
    let mut paths: Vec<_> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    paths.push(helpers.parent().unwrap().to_owned());
    static EXECUTABLE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let executable = EXECUTABLE.get_or_init(|| {
        let path = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("fix");
        std::fs::copy(root.join(env!("FIX_CLI")), &path).unwrap();
        path
    });
    let mut child = Command::new(executable)
        .args(arguments)
        .current_dir(directory)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .env_remove("RUNFILES_DIR")
        .env_remove("RUNFILES_MANIFEST_FILE")
        .env_remove("TEST_SRCDIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(status),
        "{arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    if status == 2 {
        assert!(output.stdout.is_empty());
    }
    output.stdout
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap().trim_end().into()
}

fn value(directory: &Path, arguments: &[&str]) -> String {
    text(run(directory, arguments, b"", 0))
}

#[test]
fn persistent_plumbing() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("plumbing");
    std::fs::create_dir(&directory).unwrap();
    for arguments in [&[][..], &["--help"], &["eval", "--help"]] {
        let help = value(&directory, arguments);
        assert!(help.contains("Usage:"));
    }
    assert!(value(&directory, &["flatware-run", "--help"]).contains("fix flatware-run"));
    run(&directory, &["eval", "--jobs", "invalid"], b"", 2);
    run(&directory, &["unknown-command"], b"", 2);
    run(&directory, &["create-blob"], b"", 2);
    run(&directory, &["init"], b"", 0);
    run(&directory, &["init"], b"", 0);
    assert!(run(&directory, &["label", "list"], b"", 0).is_empty());

    for length in [0, 30, 31, 131_073] {
        let bytes: Vec<u8> = (0..length).map(|index| index as u8).collect();
        let blob = text(run(&directory, &["create-blob", "-"], &bytes, 0));
        std::fs::write(directory.join("input.bin"), &bytes).unwrap();
        assert_eq!(value(&directory, &["create-blob", "input.bin"]), blob);
        assert_eq!(blob.len(), 64);
        assert_eq!(run(&directory, &["read-blob", &blob], b"", 0), bytes);
        assert_eq!(
            text(run(&directory, &["create-blob", "-"], &bytes, 0)),
            blob
        );
        assert_eq!(value(&directory, &["len", &blob]), length.to_string());
        if length <= 30 {
            assert!(!directory.join(".fix/objects").join(&blob).exists());
        }
    }

    let string = value(
        &directory,
        &["create-blob", "--string", "-l", "-l", "string"],
    );
    assert_eq!(
        run(&directory, &["read-blob", "--string", "string"], b"", 0),
        b"-l"
    );
    run(&directory, &["label", "set", "second", &string], b"", 0);
    assert_eq!(value(&directory, &["label", "list"]), "second\nstring");
    assert_eq!(value(&directory, &["label", "get", "second"]), string);
    assert_eq!(
        run(
            &directory,
            &["read-blob", "-"],
            format!("{string}\n").as_bytes(),
            0
        ),
        b"-l"
    );
    assert_eq!(value(&directory, &["resolve", "string"]), string);
    for (format, number, width) in [
        ("--u8", "255", 1),
        ("--u32", "4294967295", 4),
        ("--u64", "18446744073709551615", 8),
    ] {
        let blob = value(&directory, &["create-blob", format, number]);
        assert_eq!(
            run(&directory, &["read-blob", &blob], b"", 0),
            vec![255; width]
        );
        assert_eq!(value(&directory, &["read-blob", format, &blob]), number);
    }
    for (format, value) in [
        ("--u8", "256"),
        ("--u32", "4294967296"),
        ("--u64", "18446744073709551616"),
        ("--u64", "-1"),
    ] {
        run(&directory, &["create-blob", format, value], b"", 2);
    }
    run(&directory, &["read-blob", "--u64", "string"], b"", 2);

    std::fs::write(directory.join("first\nfile"), b"binary\0\xff").unwrap();
    std::fs::write(directory.join("second"), b"second").unwrap();
    let imported = text(run(
        &directory,
        &["create-blob", "--files-from", "-"],
        b"first\nfile\0second\0",
        0,
    ));
    let blobs: Vec<_> = imported.lines().collect();
    assert_eq!(blobs.len(), 2);
    assert_eq!(
        run(&directory, &["read-blob", blobs[0]], b"", 0),
        b"binary\0\xff"
    );
    let input_ref = value(&directory, &["create-ref", blobs[1]]);
    let environment = text(run(
        &directory,
        &["create-tree", "--bindings", "-"],
        format!(
            "input1\0{}\0input2\0{}\0file0\0{}\0",
            blobs[0], input_ref, blobs[0]
        )
        .as_bytes(),
        0,
    ));
    let root = PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap());
    run(
        &directory,
        &[
            "create-blob",
            root.join(env!("FIX_PARSER")).to_str().unwrap(),
            "-l",
            "parser",
        ],
        b"",
        0,
    );
    let graph = text(run(
        &directory,
        &["parse", "-", &environment],
        b"($input1 *[$input2 0u64 3u64] \"literal -- @ and \\\"quote\\\"\" @\"second\" $file0)",
        0,
    ));
    let children = value(&directory, &["read-tree", &graph]);
    let children: Vec<_> = children.lines().collect();
    assert_eq!(children[0], blobs[0]);
    assert_eq!(children[3], blobs[1]);
    assert_eq!(children[4], blobs[0]);
    assert_eq!(value(&directory, &["get-type", children[1]]), "encode");
    assert_eq!(
        run(&directory, &["read-blob", children[2]], b"", 0),
        b"literal -- @ and \"quote\""
    );
    let result = value(&directory, &["eval", "-j", "1", &graph]);
    let children = value(&directory, &["read-tree", &result]);
    assert_eq!(
        run(
            &directory,
            &["read-blob", children.lines().nth(1).unwrap()],
            b"",
            0
        ),
        b"sec"
    );

    let shared = text(run(&directory, &["create-blob", "-"], &[7; 64], 0));
    let unrelated = text(run(&directory, &["create-blob", "-"], &[8; 64], 0));
    let shared_ref = value(&directory, &["create-ref", &shared]);
    let root = value(
        &directory,
        &["create-tree", "-l", "exported", &shared_ref, &shared_ref],
    );
    let closure = value(&directory, &["visit", "exported"]);
    assert_eq!(closure, format!("{root}\n{shared}"));
    let archive_directory = directory.join("archives");
    std::fs::create_dir(&archive_directory).unwrap();
    run(
        &archive_directory,
        &["export", "exported", "bootstrap.tar.gz"],
        b"",
        0,
    );
    let imported = directory.join("imported");
    std::fs::create_dir(&imported).unwrap();
    run(&imported, &["init"], b"", 0);
    run(
        &imported,
        &["import", "../archives/bootstrap.tar.gz"],
        b"",
        0,
    );
    assert_eq!(value(&imported, &["visit", "exported"]), closure);
    assert_eq!(run(&imported, &["read-blob", &shared], b"", 0), [7; 64]);
    run(&imported, &["resolve", &unrelated], b"", 2);
    assert_eq!(
        std::fs::read_dir(imported.join(".fix/objects"))
            .unwrap()
            .count(),
        2
    );

    let blob = text(run(
        &directory,
        &["create-blob", "-", "-l", "input"],
        b"leaf",
        0,
    ));
    let tree = value(&directory, &["create-tree", "input", &blob, "-l", "inputs"]);
    assert_eq!(
        value(&directory, &["read-tree", "inputs"]),
        format!("{blob}\n{blob}")
    );
    run(
        &directory,
        &["label", "set", "alias with space", &blob],
        b"",
        0,
    );
    let unnamed = value(&directory, &["create-blob", "--string", "unnamed"]);
    let mixed = value(&directory, &["create-tree", &blob, &unnamed]);
    assert_eq!(
        value(&directory, &["read-tree", "--labels", &mixed]),
        format!("{blob}\t\"alias with space\"\t\"input\"\n{unnamed}")
    );
    assert_eq!(
        text(run(
            &directory,
            &["create-tree", "-"],
            format!("{blob}\n{blob}\n").as_bytes(),
            0
        )),
        tree
    );
    assert_eq!(value(&directory, &["get-type", "inputs"]), "object");
    assert_eq!(value(&directory, &["get-data-type", "inputs"]), "tree");
    assert_eq!(value(&directory, &["len", "inputs"]), "2");
    run(&directory, &["is-tag", "inputs"], b"", 1);
    run(&directory, &["is-eq", "inputs"], b"", 0);
    let reference = value(&directory, &["create-ref", "inputs"]);
    assert_eq!(value(&directory, &["get-type", &reference]), "ref");
    run(&directory, &["read-tree", &reference], b"", 2);

    let mut children = vec![tree.clone(), reference.clone()];
    for command in [
        "create-identification-thunk",
        "create-application-thunk",
        "create-selection-thunk",
    ] {
        let thunk = value(&directory, &[command, &reference]);
        assert_eq!(value(&directory, &[command, &tree]), thunk);
        assert_eq!(value(&directory, &["get-type", &thunk]), "thunk");
        run(&directory, &["is-eq", &thunk], b"", 1);
        children.push(thunk.clone());
        for kind in ["strict", "shallow"] {
            let encode = text(run(
                &directory,
                &[&format!("create-{kind}-encode"), "-"],
                format!("{thunk}\n").as_bytes(),
                0,
            ));
            assert_eq!(value(&directory, &["get-encode-type", &encode]), kind);
            run(&directory, &["label", "set", "pending", &encode], b"", 0);
            assert_eq!(value(&directory, &["resolve", "pending"]), encode);
            children.push(encode);
        }
    }
    let arguments: Vec<&str> = std::iter::once("create-tree")
        .chain(children.iter().map(String::as_str))
        .collect();
    let pending_tree = value(&directory, &arguments);
    assert_eq!(
        value(&directory, &["read-tree", &pending_tree]),
        children.join("\n")
    );
    run(&directory, &["is-eq", &pending_tree], b"", 1);
    run(&directory, &["create-application-thunk", &blob], b"", 2);
    run(&directory, &["get-data-type", &children[2]], b"", 2);
    run(&directory, &["label", "set", "../outside", &blob], b"", 2);

    let payload = std::fs::read(directory.join(".fix/objects").join(&tree)).unwrap();
    let key = format!("{}ffff", &tree[..60]);
    assert_eq!(
        std::fs::read_link(directory.join(".fix/objects").join(&tree)).unwrap(),
        PathBuf::from(format!("../data/{key}"))
    );
    let mut tagged = tree.clone().into_bytes();
    tagged[62] = b'e';
    let tagged = String::from_utf8(tagged).unwrap();
    run(&directory, &["resolve", &tagged], b"", 2);
    let mut as_blob = tree.clone().into_bytes();
    as_blob[62] = b'8';
    let as_blob = String::from_utf8(as_blob).unwrap();
    run(&directory, &["resolve", &as_blob], b"", 2);
    assert_eq!(
        text(run(&directory, &["create-blob", "-"], &payload, 0)),
        as_blob
    );
    assert_eq!(
        std::fs::read_link(directory.join(".fix/objects").join(&as_blob)).unwrap(),
        PathBuf::from(format!("../data/{key}"))
    );
    assert_eq!(run(&directory, &["read-blob", &as_blob], b"", 0), payload);
    run(&directory, &["resolve", &tagged], b"", 2);

    std::fs::remove_file(directory.join(".fix/objects").join(&as_blob)).unwrap();
    run(&directory, &["resolve", &as_blob], b"", 2);
    let data = directory.join(".fix/data").join(&key);
    let saved = directory.join("payload");
    std::fs::rename(&data, &saved).unwrap();
    std::fs::create_dir(&data).unwrap();
    run(&directory, &["read-tree", &tree], b"", 2);
    std::fs::remove_dir(&data).unwrap();
    std::fs::rename(&saved, &data).unwrap();

    let record = directory.join(".fix/objects").join(&tree);
    std::fs::remove_file(&record).unwrap();
    std::os::unix::fs::symlink("../data/missing", &record).unwrap();
    run(&directory, &["resolve", &tree], b"", 2);
    std::fs::remove_file(&record).unwrap();
    std::os::unix::fs::symlink(format!("../data/{key}"), &record).unwrap();
    std::fs::write(directory.join(".fix/data").join(&key), b"short").unwrap();
    run(&directory, &["read-tree", &tree], b"", 2);

    std::fs::remove_dir_all(directory.join(".fix/labels")).unwrap();
    std::fs::create_dir(directory.join(".fix/labels")).unwrap();
    std::fs::create_dir(directory.join(".fix/labels/blocked")).unwrap();
    run(&directory, &["label", "set", "blocked", &blob], b"", 2);
    assert!(
        std::fs::read_dir(directory.join(".fix/data"))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".tmp-"))
    );
}

#[test]
fn repository_discovery() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("discovery");
    let nested = directory.join("a/b");
    let other = directory.join("other");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(&other).unwrap();
    run(&directory, &["init"], b"", 0);
    assert_eq!(
        value(&nested, &["repository"]),
        directory.join(".fix").to_str().unwrap()
    );
    let root_blob = text(run(
        &nested,
        &["create-blob", "-", "-l", "input"],
        &[1; 64],
        0,
    ));
    assert!(!nested.join(".fix").exists());
    assert_eq!(run(&directory, &["read-blob", "input"], b"", 0), [1; 64]);

    run(&directory, &["-C", "other", "init"], b"", 0);
    assert_eq!(
        value(&nested, &["-C", "../../other", "repository"]),
        other.join(".fix").to_str().unwrap()
    );
    let other_blob = text(run(
        &directory,
        &["-C", "other", "create-blob", "-", "-l", "input"],
        &[2; 64],
        0,
    ));
    assert_ne!(other_blob, root_blob);
    assert_eq!(
        run(
            &nested,
            &["-C", "../../other", "read-blob", "input"],
            b"",
            0
        ),
        [2; 64]
    );
    assert_eq!(
        value(
            &nested,
            &["-C", directory.to_str().unwrap(), "resolve", "input"]
        ),
        root_blob
    );
    assert_eq!(
        value(
            &directory,
            &["-C", "other", "-C", "../a/b", "resolve", "input"]
        ),
        root_blob
    );
    run(&directory, &["-C"], b"", 2);
    run(&directory, &["-C", "missing", "resolve", "input"], b"", 2);

    let root = PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap());
    for (name, path) in [
        ("json", env!("FIX_JSON")),
        ("parser", env!("FIX_PARSER")),
        ("identity", env!("FIX_IDENTITY")),
    ] {
        run(
            &other,
            &["create-blob", "-", "-l", name],
            &std::fs::read(root.join(path)).unwrap(),
            0,
        );
    }
    std::fs::write(other.join("command.fix"), b"7u64").unwrap();
    run(
        &nested,
        &["-C", "../../other", "run", "command.fix"],
        b"",
        0,
    );

    let name = value(&other, &["create-blob", "--string", "payload"]);
    let binding = value(&other, &["create-tree", &name, &other_blob]);
    let environment = value(&other, &["create-tree", &binding]);
    let result = text(run(
        &other,
        &["run", "-", &environment],
        b"($payload 42u64)\n",
        0,
    ));
    let children = value(&other, &["read-tree", &result]);
    assert_eq!(children.lines().next(), Some(other_blob.as_str()));
    let number = children.lines().nth(1).unwrap();
    assert_eq!(value(&other, &["read-blob", "--u64", number]), "42");

    run(&nested, &["init"], b"", 0);
    let local_blob = text(run(
        &nested,
        &["create-blob", "-", "-l", "input"],
        &[3; 64],
        0,
    ));
    assert_eq!(run(&nested, &["read-blob", "input"], b"", 0), [3; 64]);
    assert_ne!(local_blob, root_blob);
    run(&nested, &["read-blob", &root_blob], b"", 2);
    std::os::unix::fs::symlink("a/b", directory.join("alias")).unwrap();
    assert_eq!(
        value(&directory, &["-C", "alias", "resolve", "input"]),
        local_blob
    );

    std::fs::remove_dir_all(nested.join(".fix")).unwrap();
    std::fs::write(nested.join(".fix"), b"invalid repository").unwrap();
    run(&nested, &["read-blob", "input"], b"", 2);
    std::fs::remove_file(nested.join(".fix")).unwrap();
    std::fs::create_dir(nested.join(".fix")).unwrap();
    run(&nested, &["create-blob", "-", "-l", "input"], &[4; 64], 2);
    assert_eq!(value(&directory, &["resolve", "input"]), root_blob);
}

#[test]
fn external_commands_and_filesystem_plumbing() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("helpers");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    let commands = directory.join(".fix/commands");
    std::fs::create_dir(&commands).unwrap();
    let command = commands.join("fix-location");
    std::fs::write(&command, b"#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$1\"\n").unwrap();
    std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o755)).unwrap();
    let child = directory.join("child");
    std::fs::create_dir(&child).unwrap();
    assert_eq!(
        run(&directory, &["-C", "child", "location", "argument"], b"", 0),
        format!("{}\nargument\n", child.display()).as_bytes()
    );

    let path_command = directory.join("fix-path-only");
    std::fs::write(&path_command, b"#!/bin/sh\nprintf '%s\\n' \"$1\"\n").unwrap();
    std::fs::set_permissions(&path_command, std::fs::Permissions::from_mode(0o755)).unwrap();
    let delegate = commands.join("fix-via-path");
    std::fs::write(
        &delegate,
        b"#!/bin/sh\nPATH=\"$PWD:$PATH\" exec \"$FIX\" path-only \"$1\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&delegate, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        run(&directory, &["via-path", "argument"], b"", 0),
        b"argument\n"
    );

    let root = PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap());
    run(
        &directory,
        &[
            "create-blob",
            root.join(env!("FIX_JSON")).to_str().unwrap(),
            "-l",
            "json",
        ],
        b"",
        0,
    );

    for (label, path) in [
        ("cpio", env!("FIX_CPIO")),
        ("flatware-normalize", env!("FIX_NORMALIZE")),
        ("compose", env!("FIX_COMPOSE")),
    ] {
        run(
            &directory,
            &[
                "create-blob",
                root.join(path).to_str().unwrap(),
                "-l",
                label,
            ],
            b"",
            0,
        );
    }

    let input = directory.join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::create_dir(input.join(".fix")).unwrap();
    std::fs::write(input.join(".fix/private"), b"repository metadata").unwrap();
    let filename = "binary\nfile\n";
    std::fs::write(input.join(filename), b"bytes\0with\xff").unwrap();
    symlink(filename, input.join("link")).unwrap();
    let handle = value(&directory, &["flatware-import", "input"]);
    assert_eq!(handle, value(&directory, &["flatware-import", "input"]));
    run(&directory, &["flatware-export", &handle, "output"], b"", 0);
    assert!(!directory.join("output/.fix").exists());
    assert_eq!(
        std::fs::read(directory.join("output").join(filename)).unwrap(),
        b"bytes\0with\xff"
    );
    assert_eq!(
        std::fs::read_link(directory.join("output/link")).unwrap(),
        PathBuf::from(filename)
    );
    run(&directory, &["flatware-export", &handle, "output"], b"", 2);
    let bad_name = text(run(&directory, &["create-blob", "-"], b"../escape", 0));
    let descriptor = value(&directory, &["flatware-import", "input/link"]);
    let fields = value(&directory, &["read-tree", &descriptor]);
    let fields: Vec<_> = fields.lines().collect();
    let malicious = value(
        &directory,
        &["create-tree", &bad_name, fields[1], fields[2]],
    );
    let root_fields = value(&directory, &["read-tree", &handle]);
    let root_fields: Vec<_> = root_fields.lines().collect();
    let contents = value(&directory, &["create-tree", &malicious]);
    let malformed = value(
        &directory,
        &["create-tree", root_fields[0], root_fields[1], &contents],
    );
    run(
        &directory,
        &["flatware-export", &malformed, "invalid"],
        b"",
        1,
    );
    assert!(!directory.join("escape").exists());
}

#[test]
fn json_graph_matches_parser() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("json");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    let root = PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap());
    for (label, path) in [
        ("json", env!("FIX_JSON")),
        ("parser", env!("FIX_PARSER")),
        ("identity", env!("FIX_IDENTITY")),
    ] {
        run(
            &directory,
            &[
                "create-blob",
                root.join(path).to_str().unwrap(),
                "-l",
                label,
            ],
            b"",
            0,
        );
    }
    let invalid_program = value(&directory, &["create-blob", "--string", "not an ELF"]);
    for label in ["wasm-merge", "wasm2c", "clang", "path"] {
        run(
            &directory,
            &["label", "set", label, &invalid_program],
            b"",
            0,
        );
    }
    let empty = value(&directory, &["create-tree", "-"]);
    run(&directory, &["label", "set", "toolchain", &empty], b"", 0);
    let compilation = value(&directory, &["compile", &invalid_program]);
    assert_eq!(value(&directory, &["get-type", &compilation]), "thunk");
    let identity = value(&directory, &["resolve", "identity"]);
    let values = text(run(
        &directory,
        &["json-import", "-"],
        br#"["quote\"\n\u03bb\u0000", {"hex":"00ff"}, {"u8":255}, {"u16":65535}, {"u32":4294967295}, {"u64":18446744073709551615}]"#,
        0,
    ));
    let environment = text(run(
        &directory,
        &["create-tree", "--bindings", "-"],
        format!("identity\0{identity}\0values\0{values}\0").as_bytes(),
        0,
    ));
    let json = br#"[
        {"binding":"values"},
        {"create-ref":{"binding":"values"}},
        {"create-identification-thunk":{"binding":"values"}},
        {"create-application-thunk":[{"binding":"identity"},{"binding":"values"}]},
        {"create-selection-thunk":[{"binding":"values"},{"u64":1}]},
        {"create-strict-encode":{"create-identification-thunk":{"binding":"values"}}},
        {"create-shallow-encode":{"create-identification-thunk":{"binding":"values"}}}
    ]"#;
    let graph = text(run(
        &directory,
        &["json-import", "-", &environment],
        json,
        0,
    ));
    let parsed = text(run(
        &directory,
        &["parse", "-", &environment],
        br#"values = ("quote\"\n\u03bb\u0000" 0x00ff 255u8 65535u16 4294967295u32 18446744073709551615u64)
(values &values 'values #($identity values) [values 1u64] *'values +'values)"#,
        0,
    ));
    assert_eq!(graph, parsed);
    let shared = text(run(
        &directory,
        &["json-import", "-", &environment],
        b"{\"create-strict-encode\":{\"create-identification-thunk\":{\"binding\":\"values\"}}}\n[{\"node\":0},{\"node\":0}]\n",
        0,
    ));
    let children = value(&directory, &["read-tree", &shared]);
    let children: Vec<_> = children.lines().collect();
    assert_eq!(children[0], children[1]);
    assert_eq!(value(&directory, &["get-type", children[0]]), "encode");
    let expected = value(&directory, &["create-tree", &values, &values]);
    assert_eq!(value(&directory, &["eval", "-j", "2", &shared]), expected);
    assert_eq!(
        value(&directory, &["eval", "-j", "2", &graph]),
        value(&directory, &["eval", "-j", "1", &parsed])
    );
    std::fs::write(directory.join("input\nfile"), [0, 255, 1]).unwrap();
    let file = value(&directory, &["create-blob", "input\nfile"]);
    let files = text(run(
        &directory,
        &["create-tree", "--bindings", "-"],
        format!("input\0{file}\0").as_bytes(),
        0,
    ));
    let imported = text(run(
        &directory,
        &["json-import", "-", &files],
        br#"{"binding":"input"}"#,
        0,
    ));
    assert_eq!(
        run(&directory, &["read-blob", &imported], b"", 0),
        [0, 255, 1]
    );
    let text_blob = text(run(
        &directory,
        &["json-import", "-"],
        br#""\ud83d\ude00""#,
        0,
    ));
    assert_eq!(
        run(&directory, &["read-blob", &text_blob], b"", 0),
        "😀".as_bytes()
    );
}

#[test]
fn cpio_conversion_and_normalization() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("cpio");
    let input = directory.join("input");
    std::fs::create_dir_all(input.join("nested")).unwrap();
    let payload = b"CPIO payload with binary bytes \0\xff and hard links";
    std::fs::write(input.join("a"), payload).unwrap();
    std::fs::hard_link(input.join("a"), input.join("b")).unwrap();
    std::os::unix::fs::symlink("../a", input.join("nested/link")).unwrap();
    let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(123);
    std::fs::File::open(input.join("a"))
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(time))
        .unwrap();
    run(&directory, &["init"], b"", 0);
    let root = PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap());
    for (label, path) in [
        ("cpio", env!("FIX_CPIO")),
        ("flatware-normalize", env!("FIX_NORMALIZE")),
        ("compose", env!("FIX_COMPOSE")),
    ] {
        run(
            &directory,
            &[
                "create-blob",
                root.join(path).to_str().unwrap(),
                "-l",
                label,
            ],
            b"",
            0,
        );
    }
    let apply = |procedure: &str, input: &str, jobs: &str| {
        let combination = value(&directory, &["create-tree", procedure, input]);
        let thunk = value(&directory, &["create-application-thunk", &combination]);
        let encode = value(&directory, &["create-strict-encode", &thunk]);
        value(&directory, &["eval", "-j", jobs, &encode])
    };
    let fields = |handle: &str| {
        value(&directory, &["read-tree", handle])
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let mut descriptors = Vec::new();
    for format in ["newc", "crc"] {
        let mut child = Command::new("cpio")
            .args([
                "--null",
                "-o",
                "-H",
                format,
                "--reproducible",
                "--owner=0:0",
            ])
            .current_dir(&input)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"a\0b\0nested\0nested/link\0")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let archive = text(run(&directory, &["create-blob", "-"], &output.stdout, 0));
        let descriptor = apply("cpio", &archive, "1");
        let entries = fields(&fields(&descriptor)[2]);
        let a = fields(&entries[0]);
        let b = fields(&entries[1]);
        assert_eq!(a[2], b[2]);
        assert_eq!(run(&directory, &["read-blob", &a[2]], b"", 0), payload);
        let metadata = fields(&a[1]);
        assert_eq!(
            value(&directory, &["read-blob", "--u64", &metadata[2]]),
            "2"
        );
        assert_eq!(
            value(&directory, &["read-blob", "--u64", &metadata[5]]),
            "123000000000"
        );
        let normalized = apply("flatware-normalize", &descriptor, "1");
        assert_eq!(normalized, apply("flatware-normalize", &descriptor, "4"));
        let normalized_a = fields(&fields(&fields(&normalized)[2])[0]);
        assert_eq!(normalized_a[2], a[2]);
        let metadata = fields(&normalized_a[1]);
        assert_eq!(
            value(&directory, &["read-blob", "--u64", &metadata[2]]),
            "1"
        );
        for time in &metadata[4..] {
            assert_eq!(value(&directory, &["read-blob", "--u64", time]), "0");
        }
        let composition = value(
            &directory,
            &["create-tree", "compose", "flatware-normalize", "cpio"],
        );
        assert_eq!(normalized, apply(&composition, &archive, "4"));
        let nested = value(
            &directory,
            &["create-tree", "compose", "flatware-normalize", &composition],
        );
        assert_eq!(normalized, apply(&nested, &archive, "4"));
        let packed = apply("cpio", &descriptor, "1");
        assert_eq!(descriptor, apply("cpio", &packed, "1"));
        run(
            &directory,
            &["flatware-export", &descriptor, format],
            b"",
            0,
        );
        assert_eq!(
            std::fs::read(directory.join(format).join("b")).unwrap(),
            payload
        );
        assert_eq!(
            std::fs::read_link(directory.join(format).join("nested/link")).unwrap(),
            PathBuf::from("../a")
        );
        if format == "crc" {
            let mut corrupt = output.stdout;
            let offset = corrupt
                .windows(payload.len())
                .position(|bytes| bytes == payload)
                .unwrap();
            corrupt[offset] ^= 1;
            let corrupt = text(run(&directory, &["create-blob", "-"], &corrupt, 0));
            let combination = value(&directory, &["create-tree", "cpio", &corrupt]);
            let thunk = value(&directory, &["create-application-thunk", &combination]);
            run(&directory, &["think", &thunk], b"", 1);
        }
        descriptors.push(descriptor);
    }
    assert_eq!(descriptors[0], descriptors[1]);
}

#[test]
fn user_scheduler_repository_round_trip() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("user_scheduler");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    let bytes = [42; 64];
    let blob = text(run(&directory, &["create-blob", "-"], &bytes, 0));
    run(&directory, &["label", "set", "input", &blob], b"", 0);
    let root = PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap());
    let scheduler = root.join(env!("USER_SCHEDULER"));
    let scheduler = scheduler.to_str().unwrap();
    for workers in ["1", "2"] {
        let result = value(&directory, &["user_scheduler", "input", scheduler, workers]);
        assert_eq!(value(&directory, &["read-tree", &result]), blob);
        assert_eq!(run(&directory, &["read-blob", &blob], b"", 0), bytes);
    }
}
