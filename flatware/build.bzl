"""Flatware applications construct Fix graphs; outputs read evaluated results."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load("//build:fix.bzl", "FIX_TOOL", "FixResultInfo", "FixValueInfo", "fix_action", "fix_graph", "fix_path", "fix_value")

def _archive(ctx):
    archive = ctx.actions.declare_file(ctx.label.name + "/input.cpio")
    files = []
    commands = ['mkdir "$work/root"']
    if ctx.file.src:
        if ctx.attr.subdir.startswith("/") or ".." in ctx.attr.subdir.split("/"):
            fail("subdir must be relative and cannot contain '..'")
        files.append(ctx.file.src)
        commands.append('cp -R %s/. "$work/root/"' % shell.quote(ctx.file.src.path + "/" + ctx.attr.subdir))
    for target, path in ctx.attr.inputs.items():
        if path.startswith("/") or ".." in path.split("/"):
            fail("directory input paths must be relative and cannot contain '..'")
        sources = target[DefaultInfo].files.to_list()
        for file in sources:
            destination = (path.rstrip("/") + "/" if path else "") + file.basename if len(sources) > 1 or path.endswith("/") else path
            if not destination:
                fail("directory inputs need a destination path")
            files.append(file)
            destination = '"$work/root"/' + shell.quote(destination)
            commands += ['mkdir -p "$(dirname %s)"' % destination,
                         'cp -RH %s %s' % (shell.quote(file.path), destination)]
    commands += ['archive="$PWD/"%s' % shell.quote(archive.path),
                 'cd "$work/root"',
                 'find . -exec touch -h -d @0 -- {} +',
                 'find . -name .fix -prune -o -print0 | LC_ALL=C sort -z | "$cpio" --null -o -H newc --reproducible --owner=0:0 > "$archive"']
    ctx.actions.run_shell(
        command = '''set -euo pipefail
cpio="$PWD/"%s
work=$(mktemp -d)
trap 'chmod -R u+w "$work"; rm -rf "$work"' EXIT
%s''' % (shell.quote(ctx.file._cpio.path), "\n".join(commands)),
        inputs = files,
        tools = [ctx.file._cpio],
        outputs = [archive],
        mnemonic = "FlatwareArchive",
        execution_requirements = {"no-sandbox": "1"},
    )
    return [DefaultInfo(files = depset([archive]))]

_archive_rule = rule(implementation = _archive, attrs = {
    "src": attr.label(allow_single_file = True),
    "subdir": attr.string(default = "."),
    "inputs": attr.label_keyed_string_dict(allow_files = True),
    "_cpio": attr.label(default = "@local_tools//:cpio", allow_single_file = True, cfg = "exec"),
})

def _directory_graph(ctx):
    files = [ctx.file.archive, ctx.file._compose, ctx.file._normalize, ctx.file._converter]
    bindings = [(str(ctx.label) + ":" + file.basename, file) for file in files]
    archive, compose, normalize, converter = [{"binding": key} for key, _ in bindings]
    application = {"create-application-thunk": [[compose, normalize, converter], archive]}
    directory = {"create-strict-encode": application}
    fields = [{"create-strict-encode": {"create-selection-thunk": [directory, {"u64": index}]}} for index in [1, 2]]
    return fix_graph(ctx, [ctx.attr.guest_path] + fields, files = files, imports = bindings)

_directory_rule = rule(implementation = _directory_graph, attrs = {
    "archive": attr.label(mandatory = True, allow_single_file = True),
    "guest_path": attr.string(default = "/work"),
    "_converter": attr.label(default = "//flatware:cpio_elf", allow_single_file = True),
    "_normalize": attr.label(default = "//flatware:normalize_elf", allow_single_file = True),
    "_compose": attr.label(default = "//programs/fix:compose_elf", allow_single_file = True),
})

def flatware_directory(name, src = None, subdir = ".", inputs = {}, guest_path = "/work", **kwargs):
    internal = dict(tags = kwargs.get("tags", []), testonly = kwargs.get("testonly", False), visibility = ["//visibility:private"])
    _archive_rule(name = name + "_archive", src = src, subdir = subdir, inputs = inputs, **internal)
    _directory_rule(name = name, archive = ":" + name + "_archive", guest_path = guest_path, **kwargs)

def _select(value, index):
    return {"create-strict-encode": {"create-selection-thunk": [value, {"u64": index}]}}

def _work(ctx):
    key = str(ctx.attr._result.label)
    application = {"create-application-thunk": [{"binding": key}, fix_value(ctx.attr.src), {"u64": ctx.attr.fd}]}
    return fix_graph(ctx, {"create-strict-encode": application}, [ctx.attr.src], files = [ctx.file._result], imports = [(key, ctx.file._result)])

flatware_work = rule(implementation = _work, attrs = {
    "src": attr.label(mandatory = True, providers = [FixValueInfo]),
    "fd": attr.int(default = 3),
    "_result": attr.label(default = "//flatware:result_elf", allow_single_file = True),
})

def _file(ctx):
    key = str(ctx.attr._path.label)
    lookup = {"create-application-thunk": [{"binding": key}, fix_value(ctx.attr.src), ctx.attr.path.split("/")]}
    return fix_graph(ctx, _select({"create-strict-encode": lookup}, 2), [ctx.attr.src], files = [ctx.file._path], imports = [(key, ctx.file._path)])

flatware_file = rule(implementation = _file, attrs = {
    "src": attr.label(mandatory = True, providers = [FixValueInfo]),
    "path": attr.string(mandatory = True),
    "_path": attr.label(default = "//programs/fix/rust:path_elf", allow_single_file = True),
})

def _files(ctx):
    directory_stat = [{"u64": 0}, {"u8": 3}, {"u64": 1}] + [{"u64": 0}] * 4
    file_stat = [{"u64": 0}, {"u8": 4}, {"u64": 1}] + [{"u64": 0}] * 4
    entries = [[path, file_stat, fix_value(value)] for value, path in ctx.attr.files.items()]
    return fix_graph(ctx, [ctx.attr.guest_path, directory_stat, entries], ctx.attr.files.keys())

flatware_files = rule(implementation = _files, attrs = {
    "files": attr.label_keyed_string_dict(providers = [FixValueInfo]),
    "guest_path": attr.string(default = "/work"),
})

def _application(ctx):
    dependencies = [ctx.attr.executable, ctx.attr.work] + ctx.attr.mounts.keys()
    descriptors = [["stdin", [], ""], ["stdout", [], ""], ["stderr", [], ""]]
    for target, path in ctx.attr.mounts.items():
        mount = fix_value(target)
        metadata = {"create-strict-encode": {"create-selection-thunk": [mount, {"u64": 1}]}}
        contents = {"create-strict-encode": {"create-selection-thunk": [mount, {"u64": 2}]}}
        descriptors.append([path, metadata, contents])
    descriptors.append(fix_value(ctx.attr.work))
    combination = [
        fix_value(ctx.attr.executable), [ctx.attr.argv0] + ctx.attr.args,
        ctx.attr.env, descriptors, {"u64": 0}, {"u64": 0},
    ]
    expression = {"create-strict-encode": {"create-application-thunk": combination}}
    return fix_graph(ctx, expression, dependencies)

_application_rule = rule(implementation = _application, attrs = {
    "executable": attr.label(mandatory = True, providers = [FixValueInfo]),
    "work": attr.label(mandatory = True, providers = [FixValueInfo]),
    "mounts": attr.label_keyed_string_dict(providers = [FixValueInfo]),
    "args": attr.string_list(),
    "env": attr.string_list(),
    "argv0": attr.string(default = "program"),
})

def flatware_application(name, executable, inputs = {}, work = None, **kwargs):
    if work == None:
        work = name + "_work"
        flatware_directory(name = work, inputs = inputs, tags = kwargs.get("tags", []), testonly = kwargs.get("testonly", False))
        work = ":" + work
    elif inputs:
        fail("use either inputs or work")
    _application_rule(name = name, executable = executable, work = work, **kwargs)

def _outputs(ctx):
    result = ctx.attr.src[FixResultInfo]
    for path in ctx.attr.outs:
        if path.startswith("/") or ".." in path.split("/"):
            fail("output paths must be relative and cannot contain '..'")
    outputs = [ctx.actions.declare_file(ctx.label.name + "/" + path) for path in ctx.attr.outs]
    indices = ctx.attr.indices or [-1] * len(outputs)
    if len(indices) != len(outputs):
        fail("indices must have one entry per output (-1 means the whole result)")
    commands = ['"$FIX" create-blob %s -l cpio > /dev/null' % fix_path(ctx.file._converter), 'result=$(cat %s)' % fix_path(result.handle)]
    if any([index >= 0 for index in indices]):
        commands += ['children=$("$FIX" read-tree "$result")', 'mapfile -t children <<< "$children"']
    for index in sorted(depset(indices).to_list()):
        selected = '"$result"'
        if index >= 0:
            selected = '"${children[%d]}"' % index
        commands += ['fields=$("$FIX" read-tree %s)' % selected, 'mapfile -t fields <<< "$fields"',
                     'status=$("$FIX" read-blob --u32 "${fields[1]}")',
                     'fds=$("$FIX" read-tree "${fields[0]}")', 'mapfile -t fds <<< "$fds"',
                     'if (( status != 0 )); then\n    for stream in 1 2; do\n        descriptor=$("$FIX" read-tree "${fds[stream]}")\n        mapfile -t descriptor <<< "$descriptor"\n        "$FIX" read-blob "${descriptor[2]}" >&2\n    done\n    exit 1\nfi',
                     '"$FIX" flatware-export "${fds[%d]}" output_%d' % (ctx.attr.fd, index)]
        for output, path, child in zip(outputs, ctx.attr.outs, indices):
            if child == index:
                commands += ['mkdir -p "$(dirname %s)"' % fix_path(output), 'path=$(realpath -e output_%d/%s)\n[[ $path == "$repository/output_%d/"* ]]\ncp "$path" %s' % (index, shell.quote(path), index, fix_path(output))]
    fix_action(ctx, [result.repository], depset([result.handle, ctx.file._converter]), outputs, "\n".join(commands), "FlatwareOutputs")
    return [DefaultInfo(files = depset(outputs)), OutputGroupInfo(**{path: depset([output]) for path, output in zip(ctx.attr.outs, outputs)})]

flatware_outputs = rule(implementation = _outputs, attrs = dict(FIX_TOOL,
    src = attr.label(mandatory = True, providers = [FixResultInfo]),
    outs = attr.string_list(mandatory = True),
    indices = attr.int_list(),
    fd = attr.int(default = 3),
    _converter = attr.label(default = "//flatware:cpio_elf", allow_single_file = True),
))
