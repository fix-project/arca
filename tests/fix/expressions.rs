mod directory;
use directory::Directory;

use std::path::{Path, PathBuf};
use std::process::Command;

fn runfiles() -> PathBuf {
    PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap())
}

fn seed_tool(directory: &Path, arguments: &[String]) {
    let output = Command::new(runfiles().join(env!("FIX_SEED")))
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn program(name: &str, path: &str) -> String {
    format!("{name}={}", runfiles().join(path).display())
}

fn run(directory: &Path, arguments: &[&str], input: &[u8], status: i32) -> Vec<u8> {
    use std::io::Write;
    let mut child = Command::new(runfiles().join(env!("FIX_CLI")))
        .args(arguments)
        .current_dir(directory)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
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
    output.stdout
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap().trim_end().into()
}

fn value(directory: &Path, arguments: &[&str]) -> String {
    text(run(directory, arguments, b"", 0))
}

fn bootstrap(directory: &Path, programs: &[(&str, &str)]) {
    std::fs::create_dir_all(directory).unwrap();
    let seed = runfiles().join(env!("FIX_BOOTSTRAP"));
    frontend(directory, &["init", seed.to_str().unwrap()], true);
    let mut arguments: Vec<String> = ["add", "."].map(String::from).into();
    arguments.extend(programs.iter().map(|(name, path)| program(name, path)));
    seed_tool(directory, &arguments);
}

fn frontend(directory: &Path, arguments: &[&str], success: bool) -> std::process::Output {
    let output = Command::new(runfiles().join(env!("FIX_CLI")))
        .args(arguments)
        .current_dir(directory)
        .env("FIX_JOBS", "1")
        .env("FIX_TRUST", directory.join(".fix/trust"))
        .env_remove("FIX_SCHEDULER")
        .env_remove("FIX_COLLECTOR")
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .output()
        .unwrap();
    assert_eq!(
        output.status.success(),
        success,
        "{arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn expression(directory: &Path, arguments: &[&str]) -> String {
    text(frontend(directory, arguments, true).stdout)
}

fn handle(name: &str) -> String {
    format!("@handle(\"{name}\")")
}

#[test]
fn scripts_import_relative_files_resolve_labels_and_save_results() {
    let directory = Directory::new("scripts");
    bootstrap(&directory, &[("procedure-name", env!("FIX_IDENTITY"))]);
    std::fs::create_dir(directory.join("recipes")).unwrap();
    std::fs::write(directory.join("recipes/input.bin"), [0, 255, 42]).unwrap();
    std::fs::write(
        directory.join("recipes/programλ.fix"),
        r#"
-- @"missing" $missing
{- @("ignored" "bad name") -}
input = @file("input.bin")
*[*#("" @label("procedure-name")-- ignored @"missing"
(input "@\"literal\" $literal" 42u64)) 2u64]
"#,
    )
    .unwrap();
    let parsed = expression(&directory, &["parse", "recipes/programλ.fix"]);
    assert_eq!(value(&directory, &["get-type", &parsed]), "encode");
    let result = expression(
        &directory,
        &[
            "-j",
            "2",
            "--memory",
            "1024",
            "-l",
            "result",
            "eval",
            &handle(&parsed),
        ],
    );
    assert_eq!(value(&directory, &["resolve", "result"]), result);
    assert_eq!(
        expression(&directory, &["run", "recipes/programλ.fix"]),
        result
    );
    let fields = value(&directory, &["read-tree", &result]);
    let fields: Vec<_> = fields.lines().collect();
    assert_eq!(
        run(&directory, &["read-blob", fields[0]], b"", 0),
        [0, 255, 42]
    );
    assert_eq!(
        run(&directory, &["read-blob", fields[1]], b"", 0),
        b"@\"literal\" $literal"
    );
    assert_eq!(value(&directory, &["read-blob", "--u64", fields[2]]), "42");
}

#[test]
fn script_imports_are_scoped_lazy_and_relative_to_their_file() {
    let directory = Directory::new("script-imports");
    bootstrap(&directory, &[("identity", env!("FIX_IDENTITY"))]);
    std::fs::create_dir_all(directory.join("recipes/modules")).unwrap();
    std::fs::write(
        directory.join("recipes/modules/payload.bin"),
        b"module payload",
    )
    .unwrap();
    std::fs::write(directory.join("recipes/parent.bin"), b"caller file").unwrap();
    std::fs::write(directory.join("recipes/modules/external.fix"), "$value\n").unwrap();
    std::fs::write(
        directory.join("recipes/modules/value.fix"),
        r#"
local = "inside"
payload = @file("payload.bin")
*[*#("" $procedure (local payload $message)) 2u64]
"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("recipes/main.fix"),
        r#"
local = "outside"
imported = @script("modules/value.fix" message=local)
inherited = @script("modules/value.fix")
(imported local imported inherited @script("modules/external.fix" value=@file("parent.bin")))
"#,
    )
    .unwrap();
    let parsed = expression(
        &directory,
        &[
            "-b",
            "procedure=@label(\"identity\")",
            "-b",
            "message=\"global\"",
            "parse",
            "recipes/main.fix",
        ],
    );
    let fields = value(&directory, &["read-tree", &parsed]);
    let fields: Vec<_> = fields.lines().collect();
    assert_eq!(value(&directory, &["get-type", fields[0]]), "encode");
    let result = expression(&directory, &["-j", "2", "eval", &handle(&parsed)]);
    let fields = value(&directory, &["read-tree", &result]);
    let fields: Vec<_> = fields.lines().collect();
    assert_eq!(fields[0], fields[2]);
    assert_eq!(
        run(&directory, &["read-blob", fields[1]], b"", 0),
        b"outside"
    );
    let imported = value(&directory, &["read-tree", fields[0]]);
    let imported: Vec<_> = imported.lines().collect();
    assert_eq!(
        run(&directory, &["read-blob", imported[0]], b"", 0),
        b"inside"
    );
    assert_eq!(
        run(&directory, &["read-blob", imported[1]], b"", 0),
        b"module payload"
    );
    assert_eq!(
        run(&directory, &["read-blob", imported[2]], b"", 0),
        b"outside"
    );
    let inherited = value(&directory, &["read-tree", fields[3]]);
    let inherited: Vec<_> = inherited.lines().collect();
    assert_eq!(
        run(&directory, &["read-blob", inherited[2]], b"", 0),
        b"global"
    );
    assert_eq!(
        run(&directory, &["read-blob", fields[4]], b"", 0),
        b"caller file"
    );
    std::fs::write(directory.join("recipes/a.fix"), "@script(\"b.fix\")\n").unwrap();
    std::fs::write(directory.join("recipes/b.fix"), "@script(\"a.fix\")\n").unwrap();
    let recursive = frontend(&directory, &["parse", "recipes/a.fix"], false);
    assert!(String::from_utf8_lossy(&recursive.stderr).contains("recursive script import"));
}

#[test]
fn expressions_are_used_for_cli_values_and_script_arguments() {
    let directory = Directory::new("expressions");
    bootstrap(&directory, &[("identity", env!("FIX_IDENTITY"))]);
    let number = expression(&directory, &["42u64"]);
    assert_eq!(value(&directory, &["read-blob", "--u64", &number]), "42");
    std::fs::create_dir(directory.join("recipes")).unwrap();
    std::fs::write(directory.join("recipes/input.bin"), b"local input").unwrap();
    std::fs::write(directory.join("recipes/leaf.fix"), "$message\n").unwrap();
    std::fs::write(
        directory.join("recipes/child.fix"),
        r#"
($0 $message @script("leaf.fix") @script("leaf.fix" message="inner") @file("input.bin"))
"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("recipes/main.fix"),
        r#"
@script("child.fix" $0 message=$message)
"#,
    )
    .unwrap();
    let result = expression(
        &directory,
        &[
            "-l",
            "result",
            "@script(\"recipes/main.fix\" 7u64 message=\"outer\")",
        ],
    );
    let fields = value(&directory, &["read-tree", &result]);
    let fields: Vec<_> = fields.lines().collect();
    assert_eq!(value(&directory, &["read-blob", "--u64", fields[0]]), "7");
    for (field, expected) in
        fields[1..]
            .iter()
            .zip([b"outer".as_slice(), b"outer", b"inner", b"local input"])
    {
        assert_eq!(run(&directory, &["read-blob", field], b"", 0), expected);
    }
    assert_eq!(expression(&directory, &["@label(\"result\")"]), result);
    assert_eq!(expression(&directory, &[&handle(&result)]), result);
    std::fs::write(
        directory.join("recipes/child.fix"),
        "@script(\"positional.fix\")\n",
    )
    .unwrap();
    std::fs::write(directory.join("recipes/positional.fix"), "$0\n").unwrap();
    let missing = frontend(
        &directory,
        &["@script(\"recipes/main.fix\" 7u64 message=\"outer\")"],
        false,
    );
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing positional argument $0"));
    let missing = frontend(&directory, &["@script(\"recipes/leaf.fix\")"], false);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing named argument $message"));
    frontend(
        &directory,
        &["@script(\"recipes/leaf.fix\" 7u64 message=\"outer\")"],
        false,
    );
    let inherited = expression(
        &directory,
        &["-b", "message=\"CLI\"", "@script(\"recipes/leaf.fix\")"],
    );
    assert_eq!(run(&directory, &["read-blob", &inherited], b"", 0), b"CLI");
    let unforced = expression(&directory, &["#(\"\" @label(\"identity\") 9u64)"]);
    assert_eq!(value(&directory, &["get-type", &unforced]), "thunk");
    let forced = expression(&directory, &["*#(\"\" @label(\"identity\") 9u64)"]);
    assert_eq!(value(&directory, &["get-type", &forced]), "object");
    let graph = expression(&directory, &["(1u64 *#(\"\" @label(\"identity\") 2u64))"]);
    let children = value(&directory, &["read-tree", &graph]);
    let children: Vec<_> = children.lines().collect();
    assert_eq!(value(&directory, &["get-type", children[1]]), "encode");
    assert_eq!(
        expression(&directory, &["think", "#(\"\" @label(\"identity\") 9u64)"]),
        expression(&directory, &["(\"\" @label(\"identity\") 9u64)"])
    );
    let forty_two = value(&directory, &["create-blob", "--u64", "42"]);
    let forty_three = value(&directory, &["create-blob", "--u64", "43"]);
    run(
        &directory,
        &["label", "set", &forty_two, &forty_three],
        b"",
        0,
    );
    assert_eq!(expression(&directory, &[&handle(&forty_two)]), forty_two);
    assert_eq!(
        expression(&directory, &[&format!("@label(\"{forty_two}\")")]),
        forty_three
    );
}

#[test]
fn compilation_uses_the_compile_runnable() {
    let directory = Directory::new("compile-command");
    bootstrap(&directory, &[]);
    let wasm = runfiles().join(env!("FIX_ADD_MODULE"));
    let add = expression(
        &directory,
        &[
            "-j",
            "4",
            "compile",
            &format!("@file({:?})", wasm.to_str().unwrap()),
            "2",
        ],
    );
    run(&directory, &["is-tag", &add], b"", 0);
    let fields = value(&directory, &["read-tree", &add]);
    let fields: Vec<_> = fields.lines().collect();
    assert_eq!(
        fields[0],
        value(&directory, &["resolve", "trusted-compiler"])
    );
    let sum = expression(
        &directory,
        &[&format!("*#(\"\" {} 2u64 3u64)", handle(&add))],
    );
    assert_eq!(value(&directory, &["read-blob", "--u64", &sum]), "5");
    let error = expression(&directory, &["compile", "\"not WebAssembly\""]);
    let fields = value(&directory, &["read-tree", &error]);
    assert_eq!(
        run(
            &directory,
            &["read-blob", fields.lines().nth(2).unwrap()],
            b"",
            0
        ),
        b"Error"
    );
}

#[test]
fn archives_and_parsed_environments_round_trip() {
    let directory = Directory::new("archives");
    bootstrap(&directory, &[]);
    std::fs::write(directory.join("first\nfile"), b"binary\0\xff").unwrap();
    std::fs::write(directory.join("second"), b"second").unwrap();
    let first = value(&directory, &["create-blob", "first\nfile"]);
    let second = value(&directory, &["create-blob", "second"]);
    let input_ref = value(&directory, &["create-ref", &second]);
    let input2 = format!("input2={}", handle(&input_ref));
    let source =
        "($input1 *[$input2 0u64 3u64] \"literal -- @ and \\\"quote\\\"\" @file(\"second\"))";
    std::fs::write(directory.join("graph.fix"), source).unwrap();
    let graph = expression(
        &directory,
        &[
            "parse",
            "graph.fix",
            "-b",
            &format!("input1={}", handle(&first)),
            "-b",
            &input2,
        ],
    );
    let children = value(&directory, &["read-tree", &graph]);
    let children: Vec<_> = children.lines().collect();
    assert_eq!(children[0], first);
    assert_eq!(children[3], second);
    assert_eq!(value(&directory, &["get-type", children[1]]), "encode");
    assert_eq!(
        run(&directory, &["read-blob", children[2]], b"", 0),
        b"literal -- @ and \"quote\""
    );
    let result = expression(&directory, &["eval", &handle(&graph)]);
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
        &["-l", "exported", "create-tree", &shared_ref, &shared_ref],
    );
    std::fs::create_dir(directory.join("archives")).unwrap();
    frontend(
        &directory,
        &["export", "@label(\"exported\")", "archives/exported.tar.gz"],
        true,
    );
    frontend(
        &directory,
        &["export", "@label(\"exported\")", "archives/exported.tar.gz"],
        false,
    );
    let imported = directory.join("imported");
    std::fs::create_dir(&imported).unwrap();
    run(&imported, &["init"], b"", 0);
    run(
        &imported,
        &["import", "../archives/exported.tar.gz"],
        b"",
        0,
    );
    let visited = value(&imported, &["visit", &root]);
    assert_eq!(
        visited.lines().collect::<std::collections::BTreeSet<_>>(),
        [root.as_str(), shared.as_str()].into_iter().collect()
    );
    assert_eq!(run(&imported, &["read-blob", &shared], b"", 0), [7; 64]);
    run(&imported, &["resolve", &unrelated], b"", 2);
}

#[test]
fn flatware_directories_round_trip_through_native_import_and_export() {
    use std::os::unix::fs::symlink;
    let directory = Directory::new("flatware");
    bootstrap(&directory, &[]);
    let input = directory.join("input");
    std::fs::create_dir_all(input.join("nested/deeper")).unwrap();
    std::fs::create_dir(input.join(".fix")).unwrap();
    std::fs::write(input.join(".fix/private"), b"repository metadata").unwrap();
    std::fs::create_dir(input.join("nested/.fix")).unwrap();
    let filename = "binary\nfile\n";
    let payload = b"bytes\0with\xff";
    std::fs::write(input.join(filename), payload).unwrap();
    std::fs::hard_link(input.join(filename), input.join("hard")).unwrap();
    std::fs::write(input.join("nested/deeper/z"), b"z").unwrap();
    std::fs::write(input.join("nested/deeper/a"), b"a").unwrap();
    symlink("../binary\nfile\n", input.join("nested/link")).unwrap();
    let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(123);
    std::fs::File::open(input.join("hard"))
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(time))
        .unwrap();
    let fields = |handle: &str| {
        value(&directory, &["read-tree", handle])
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let numbers = |stat: &str| {
        fields(stat)
            .iter()
            .enumerate()
            .map(|(index, field)| {
                if index == 1 {
                    run(&directory, &["read-blob", field], b"", 0)[0].to_string()
                } else {
                    value(&directory, &["read-blob", "--u64", field])
                }
            })
            .collect::<Vec<_>>()
    };
    let root = value(&directory, &["-l", "root", "flatware-import", "input"]);
    assert_eq!(value(&directory, &["resolve", "root"]), root);
    assert_eq!(
        value(&directory, &["flatware-import", "input", "input"]),
        root
    );
    let descriptor = fields(&root);
    assert_eq!(
        run(&directory, &["read-blob", &descriptor[0]], b"", 0),
        b"input"
    );
    assert_eq!(numbers(&descriptor[1]), ["0", "3", "1", "3", "0", "0", "0"]);
    let entries: Vec<_> = fields(&descriptor[2])
        .iter()
        .map(|entry| fields(entry))
        .collect();
    let names: Vec<_> = entries
        .iter()
        .map(|entry| text(run(&directory, &["read-blob", &entry[0]], b"", 0)))
        .collect();
    assert_eq!(names, ["binary\nfile", "hard", "nested"]);
    assert_eq!(entries[0][2], entries[1][2]);
    assert_eq!(
        run(&directory, &["read-blob", &entries[0][2]], b"", 0),
        payload
    );
    assert_eq!(
        numbers(&entries[1][1]),
        ["0", "4", "1", &payload.len().to_string(), "0", "0", "0"]
    );
    let nested: Vec<_> = fields(&entries[2][2])
        .iter()
        .map(|entry| fields(entry))
        .collect();
    assert_eq!(nested.len(), 2);
    assert_eq!(numbers(&nested[1][1])[1], "7");
    assert_eq!(
        run(&directory, &["read-blob", &nested[1][2]], b"", 0),
        b"../binary\nfile\n"
    );
    let deeper: Vec<_> = fields(&nested[0][2])
        .iter()
        .map(|entry| fields(entry))
        .collect();
    assert_eq!(run(&directory, &["read-blob", &deeper[0][0]], b"", 0), b"a");
    let single = value(
        &directory,
        &["flatware-import", "input/nested/link", "link"],
    );
    assert_eq!(fields(&single)[1], nested[1][1]);
    expression(
        &directory,
        &["flatware-export", "@label(\"root\")", "output"],
    );
    assert!(!directory.join("output/.fix").exists());
    assert_eq!(
        std::fs::read(directory.join("output").join(filename)).unwrap(),
        payload
    );
    assert_eq!(
        std::fs::read(directory.join("output/hard")).unwrap(),
        payload
    );
    assert_eq!(
        std::fs::read(directory.join("output/nested/deeper/z")).unwrap(),
        b"z"
    );
    assert_eq!(
        std::fs::read_link(directory.join("output/nested/link")).unwrap(),
        PathBuf::from("../binary\nfile\n")
    );
    assert_eq!(
        value(&directory, &["flatware-import", "output", "input"]),
        root
    );
    frontend(
        &directory,
        &["flatware-export", &handle(&root), "output"],
        false,
    );
    let bad_name = text(run(&directory, &["create-blob", "-"], b"../escape", 0));
    let malicious = value(
        &directory,
        &["create-tree", &bad_name, &entries[0][1], &entries[0][2]],
    );
    let contents = value(&directory, &["create-tree", &malicious]);
    let malformed = value(
        &directory,
        &["create-tree", &descriptor[0], &descriptor[1], &contents],
    );
    frontend(
        &directory,
        &["flatware-export", &handle(&malformed), "invalid"],
        false,
    );
    assert!(!directory.join("escape").exists());
}

#[test]
fn evaluation_rejects_scheduler_authored_coupons() {
    use fixapi::Fix;
    let directory = Directory::new("untrusted_scheduler");
    bootstrap(&directory, &[("untrusted", env!("FIX_TAGGER"))]);
    let mut client = fixcli::Client::open(
        &directory,
        fixcli::Options {
            scheduler: "untrusted".into(),
            trust: Some(directory.join(".fix/trust")),
            ..fixcli::Options::default()
        },
    )
    .unwrap();
    let untrusted = client.lookup("untrusted").unwrap();
    let collector = client.lookup("collector").unwrap();
    for mode in [4u64, 5u64] {
        let input = client.blob(&mode.to_le_bytes()).unwrap();
        let eval = client.blob(b"eval").unwrap();
        let request = client
            .tree(&[eval, untrusted.clone(), collector.clone(), input.clone()])
            .unwrap();
        let coupon = client.schedule(&request).unwrap();
        assert!(client.is_tag(&coupon).unwrap());
        assert!(matches!(
            client.eval(&input),
            Err(fixcli::Error::Execution(_))
        ));
    }
}

#[test]
fn file_imports_directory_contents() {
    let directory = Directory::new("directories");
    bootstrap(&directory, &[]);
    std::fs::create_dir_all(directory.join("recipes/tree/sub")).unwrap();
    std::fs::write(directory.join("recipes/tree/a.txt"), "alpha").unwrap();
    std::fs::write(directory.join("recipes/tree/sub/b.bin"), [1, 2]).unwrap();
    std::fs::write(directory.join("recipes/tree.fix"), "@file(\"tree\")").unwrap();
    let tree = expression(&directory, &["run", "recipes/tree.fix"]);
    let child = |name: &str, path: &[usize]| {
        path.iter().fold(name.to_owned(), |name, index| {
            value(&directory, &["read-tree", &name])
                .lines()
                .nth(*index)
                .unwrap()
                .to_owned()
        })
    };
    let blob = |path: &[usize]| run(&directory, &["read-blob", &child(&tree, path)], b"", 0);
    assert_eq!(value(&directory, &["read-tree", &tree]).lines().count(), 2);
    assert_eq!(blob(&[0, 0]), b"a.txt");
    assert_eq!(blob(&[0, 2]), b"alpha");
    assert_eq!(blob(&[1, 0]), b"sub");
    assert_eq!(blob(&[1, 2, 0, 2]), [1, 2]);
}
