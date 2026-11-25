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
        .env("FIX_UNSIGNED", "true")
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

fn command(directory: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new(runfiles().join(env!("FIX_CLI")));
    command
        .args(arguments)
        .env("FIX_UNSIGNED", "true")
        .env("FIX_CONFIG_DIR", directory.join("config"))
        .env("XDG_DATA_HOME", directory.join("data"))
        .env("FIX_JOBS", "1")
        .env("FIX_TRUST", directory.join(".fix/trust"))
        .env("FIX_SCHEDULER", "scheduler")
        .env("FIX_COLLECTOR", "collector")
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .current_dir(directory);
    command
}

fn run(directory: &Path, arguments: &[&str], input: &[u8], status: i32) -> Vec<u8> {
    use std::io::Write;
    let mut child = command(directory, arguments)
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
    frontend(directory, &["init"], true);
    frontend(directory, &["import", seed.to_str().unwrap()], true);
    frontend(directory, &["trust", "add-tagger", "collector"], true);
    let mut arguments: Vec<String> = ["add", "."].map(String::from).into();
    arguments.extend(programs.iter().map(|(name, path)| program(name, path)));
    seed_tool(directory, &arguments);
}

fn frontend(directory: &Path, arguments: &[&str], success: bool) -> std::process::Output {
    let output = command(directory, arguments).output().unwrap();
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
*[*#(268435456u64 @label("procedure-name")-- ignored @"missing"
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
            "4096",
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
fn run_uses_a_shebang_expression_to_apply_a_callable_to_the_entire_file() {
    let directory = Directory::new("shebang");
    bootstrap(&directory, &[("identity", env!("FIX_IDENTITY"))]);
    std::fs::create_dir(directory.join("scripts")).unwrap();
    std::fs::write(
        directory.join("scripts/interpreter.fix"),
        "*[$interpreter 0u64]",
    )
    .unwrap();
    let source = b"#! *#($limit @script(\"interpreter.fix\") $0)\r\n\xff\0script body\n";
    let path = "scripts/program";
    std::fs::write(directory.join(path), source).unwrap();
    let binding = "interpreter=(@label(\"identity\"))";
    let from_file = expression(&directory, &["-b", binding, "run", path]);
    std::fs::copy(
        directory.join("scripts/interpreter.fix"),
        directory.join("interpreter.fix"),
    )
    .unwrap();
    let from_stdin = text(run(&directory, &["-b", binding, "run", "-"], source, 0));
    assert_eq!(from_file, from_stdin);
    let fields = value(&directory, &["read-tree", &from_file]);
    let [memory, _, input] = fields.lines().collect::<Vec<_>>().try_into().unwrap();
    assert_eq!(
        value(&directory, &["read-blob", "--u64", memory]),
        "536870912"
    );
    assert_eq!(run(&directory, &["read-blob", input], b"", 0), source);

    let result = expression(&directory, &["-b", binding, "run", "--limit=2G", path]);
    let fields = value(&directory, &["read-tree", &result]);
    let memory = fields.lines().next().unwrap();
    assert_eq!(
        value(&directory, &["read-blob", "--u64", memory]),
        "2147483648"
    );

    let output = frontend(&directory, &["-b", binding, "run", "--limit=1", path], true);
    assert!(String::from_utf8_lossy(&output.stderr).contains("memory limit exceeded (1 bytes)"));
    let result = text(output.stdout);
    let fields = value(&directory, &["read-tree", &result]);
    let [marker, limit] = fields.lines().collect::<Vec<_>>().try_into().unwrap();
    assert_eq!(value(&directory, &["read-blob", marker]), "Fix.memory");
    assert_eq!(value(&directory, &["read-blob", "--u64", limit]), "1");

    let bare = b"#! @label(\"identity\")\n";
    let result = text(run(&directory, &["run", "-"], bare, 0));
    assert_eq!(
        result,
        expression(&directory, &["eval", r#"@label("identity")"#])
    );
}

#[test]
fn run_passes_positional_fix_expressions_to_scripts_and_shebang_callables() {
    let directory = Directory::new("run-arguments");
    bootstrap(&directory, &[("identity", env!("FIX_IDENTITY"))]);
    std::fs::create_dir(directory.join("scripts")).unwrap();
    std::fs::write(directory.join("input.bin"), b"caller directory").unwrap();
    let source = b"($0 $1)";
    std::fs::write(directory.join("scripts/program.fix"), source).unwrap();
    let arguments = ["$number", r#"@file("input.bin")"#];
    let binding = "number=42u64";
    let script = expression(
        &directory,
        &[
            &["-b", binding, "run", "scripts/program.fix"],
            arguments.as_slice(),
        ]
        .concat(),
    );
    let stdin = text(run(
        &directory,
        &[&["-b", binding, "run", "-"], arguments.as_slice()].concat(),
        source,
        0,
    ));
    assert_eq!(stdin, script);
    let fields = value(&directory, &["read-tree", &script]);
    let [number, input] = fields.lines().collect::<Vec<_>>().try_into().unwrap();
    assert_eq!(value(&directory, &["read-blob", "--u64", number]), "42");
    assert_eq!(value(&directory, &["read-blob", input]), "caller directory");

    let source = b"#! *#($limit @label(\"identity\") $0 $2 \"fixed\" $1)\n\xff";
    std::fs::write(directory.join("scripts/program"), source).unwrap();
    let result = expression(
        &directory,
        &[
            &["-b", binding, "run", "scripts/program"],
            arguments.as_slice(),
        ]
        .concat(),
    );
    let fields = value(&directory, &["read-tree", &result]);
    let [_, _, body, arg1, fixed, arg0] = fields.lines().collect::<Vec<_>>().try_into().unwrap();
    assert_eq!(run(&directory, &["read-blob", body], b"", 0), source);
    assert_eq!(value(&directory, &["read-blob", fixed]), "fixed");
    assert_eq!((arg0, arg1), (number, input));
    let imported = expression(
        &directory,
        &[
            "-b",
            binding,
            "eval",
            r#"@script("scripts/program" $number @file("input.bin"))"#,
        ],
    );
    assert_eq!(imported, result);
}

#[test]
fn run_reports_conventional_fix_error_trees() {
    let directory = Directory::new("run-errors");
    bootstrap(&directory, &[("identity", env!("FIX_IDENTITY"))]);
    for (source, message) in [
        (
            r#"("Fix.error" "invalid input")"#,
            "Fix error: invalid input",
        ),
        (r#"("Rust.panic" "oops")"#, "Rust panic: oops"),
        (r#"("Wasm.trap" 2u32)"#, "Wasm trap code 2"),
    ] {
        std::fs::write(directory.join("error.fix"), source).unwrap();
        let output = frontend(&directory, &["run", "error.fix"], true);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(message), "{stderr}");
        let parsed = expression(&directory, &["parse", "error.fix"]);
        assert_eq!(text(output.stdout), parsed);
    }
    std::fs::write(directory.join("data.fix"), r#"("Wasm.trap" "other data")"#).unwrap();
    let result = expression(&directory, &["run", "data.fix"]);
    assert_eq!(
        value(&directory, &["read-tree", &result]).lines().count(),
        2
    );
}

#[test]
fn run_reports_the_request_in_an_unhandled_fix_error_effect() {
    let directory = Directory::new("run-effect-error");
    bootstrap(&directory, &[("effects", env!("FIX_LINEAR_EFFECTS"))]);
    std::fs::write(
        directory.join("effect.fix"),
        r#"*#(2147483648u64 @label("effects") ("Fix.error" "effect failed") 0u64)"#,
    )
    .unwrap();
    let output = frontend(
        &directory,
        &["--memoize", "never", "run", "effect.fix"],
        true,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Fix error: effect failed"), "{stderr}");
    let handle = text(output.stdout);
    let fields = value(&directory, &["read-tree", &handle]);
    let [request, continuation] = fields.lines().collect::<Vec<_>>().try_into().unwrap();
    let request = value(&directory, &["read-tree", request]);
    let [marker, message] = request.lines().collect::<Vec<_>>().try_into().unwrap();
    assert_eq!(value(&directory, &["read-blob", marker]), "Fix.error");
    assert_eq!(value(&directory, &["read-blob", message]), "effect failed");
    let continuation = value(&directory, &["read-tree", continuation]);
    let marker = continuation.lines().nth(2).unwrap();
    assert_eq!(value(&directory, &["read-blob", marker]), "Continuation");
}

#[test]
fn run_previews_short_results_using_fix_notation() {
    let directory = Directory::new("run-preview");
    bootstrap(&directory, &[("identity", env!("FIX_IDENTITY"))]);
    expression(&directory, &["-l", "greeting", "eval", r#""wxyz""#]);
    for (source, preview) in [
        (r#""hello""#, r#""hello""#),
        (r#""A""#, r#""A" / 65 (0x41)"#),
        ("0xff", "-1 (0xff)"),
        ("0x00ff", "-256 (0xff00)"),
        (r#""abcd""#, r#""abcd" / 1684234849 (0x64636261)"#),
        ("42u32", r#""*\0\0\0" / 42 (0x0000002a)"#),
        ("0xffffffff", "-1 (0xffffffff)"),
        ("0xfeffffffffffffff", "-2 (0xfffffffffffffffe)"),
        (r#"@label("identity")"#, "identity"),
        (
            r#""wxyz""#,
            r#"greeting = "wxyz" / 2054781047 (0x7a797877)"#,
        ),
        (
            r#"("hello" 0xffffffff ("café" @label("identity")))"#,
            r#"("hello" -1 (0xffffffff) ("café" identity))"#,
        ),
        (r#"&("hello" "world")"#, r#"&("hello" "world")"#),
        (r#"'&("hello" "world")"#, r#"'("hello" "world")"#),
        (r#"#("hello" @label("identity"))"#, r#"#("hello" identity)"#),
        (
            r#"#(*'&("hello") +'&("world"))"#,
            r#"#(*'("hello") +'("world"))"#,
        ),
        (
            r#"[("hello" "world") 0u64]"#,
            r#"[("hello" "world") "\0\0\0\0\0\0\0\0" / 0 (0x0000000000000000)]"#,
        ),
    ] {
        std::fs::write(directory.join("preview.fix"), source).unwrap();
        let output = frontend(&directory, &["run", "preview.fix"], true);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr
                .lines()
                .any(|line| line == format!("fix: result {preview}")),
            "{source}: {stderr}"
        );
        let result = text(output.stdout);
        assert_eq!(result.len(), 64);
    }
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
*[*#(268435456u64 $procedure (local payload $message)) 2u64]
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
    let unforced = expression(&directory, &["#(268435456u64 @label(\"identity\") 9u64)"]);
    assert_eq!(value(&directory, &["get-type", &unforced]), "thunk");
    let forced = expression(&directory, &["*#(268435456u64 @label(\"identity\") 9u64)"]);
    assert_eq!(value(&directory, &["get-type", &forced]), "object");
    let graph = expression(
        &directory,
        &["(1u64 *#(268435456u64 @label(\"identity\") 2u64))"],
    );
    let children = value(&directory, &["read-tree", &graph]);
    let children: Vec<_> = children.lines().collect();
    assert_eq!(value(&directory, &["get-type", children[1]]), "encode");
    assert_eq!(
        expression(
            &directory,
            &["think", "#(268435456u64 @label(\"identity\") 9u64)"]
        ),
        expression(&directory, &["(268435456u64 @label(\"identity\") 9u64)"])
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
        &[&format!("*#(268435456u64 {} 2u64 3u64)", handle(&add))],
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
