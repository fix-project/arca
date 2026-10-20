"""Self-hosted generations expressed as Fix graphs."""

load("//build:fix.bzl", "FIX_EVAL", "FixResultInfo", "FixValueInfo", "fix_action", "fix_binding", "fix_budgets", "fix_graph_commands", "fix_graph_description", "fix_path", "fix_select_index", "fix_stored", "fix_strict_encode", "fix_tree")
load("//flatware:build.bzl", "flatware_application", "flatware_directory", "flatware_file", "flatware_files", "flatware_work")
load(":native.bzl", "CFLAGS", "COMMANDS")
load(":configuration.bzl", "ALLOW_TRANSITION", "mode", "stage")

_FLAGS = [flag.replace("{usr}", "/usr") for flag in CFLAGS]
_ENV = ["TMPDIR=/work", "LC_ALL=C", "SOURCE_DATE_EPOCH=0", "BINARYEN_CORES=1"]
_TAGS = ["manual"]

def _phase(name, requests):
    fix_tree(name = name + "_requests", children = requests, tags = _TAGS)
    fix_binding(name = name + "_eval", value = name + "_requests", tags = _TAGS)
    return ":" + name + "_eval"

def _generation_impl(ctx):
    jobs, memory = fix_budgets(ctx)
    repository = ctx.actions.declare_file(ctx.label.name + "/result.tar")
    handle = ctx.actions.declare_file(ctx.label.name + "/result.handle")
    executables = [ctx.actions.declare_file(ctx.attr.directory + "/" + command + "/" + command + ".elf") for command in COMMANDS]
    commands = [': > results']
    values = [phase[FixValueInfo] for phase in ctx.attr.phases]
    sources = []
    for phase, value in zip(ctx.attr.phases, values):
        source = fix_graph_description(ctx, value, phase.label.name)
        sources.append(source)
        commands += ['echo %s >&2' % phase.label.name]
        commands += fix_graph_commands(ctx, value, source, "graph.handle", bindings = "results")
        commands += ['"$FIX" eval --memoize --memory %d -j %d "$(cat graph.handle)" > phase.handle' % (memory, jobs),
                     'printf "%%s\\0%%s\\0" %s "$(cat phase.handle)" >> results' % phase.label.name]
    commands += ['children=$("$FIX" read-tree "$(cat phase.handle)")',
                 'mapfile -t children <<< "$children"']
    commands += ['"$FIX" read-blob "${children[%d]}" > %s' % (index, fix_path(output)) for index, output in enumerate(executables)]
    commands += [': > certificates']
    for index, command in enumerate(COMMANDS):
        commands += ['certificate=$(cat tools/%s)' % command,
                     'fields=$("$FIX" read-tree "$certificate")',
                     'mapfile -t fields <<< "$fields"',
                     '[ "${children[%d]}" = "${fields[1]}" ] || { echo "%s: rebuilt ELF differs from certified ELF" >&2; exit 1; }' % (index, command),
                     'printf "%s\\n" "$certificate" >> certificates']
    commands += ['"$FIX" create-tree - < certificates > %s' % fix_path(handle),
                 '"$FIX" export "$(cat %s)" %s' % (fix_path(handle), fix_path(repository))]
    inputs = depset(sources, transitive = [value.inputs for value in values])
    repositories = depset(transitive = [value.repositories for value in values]).to_list()
    fix_action(ctx, repositories, inputs, [repository, handle] + executables, "\n".join(commands), "BootstrapFix", jobs = jobs, memory = memory, stored = [handle])
    return fix_stored(ctx, repository, handle) + [FixResultInfo(handle = handle, repository = repository), OutputGroupInfo(**{command: depset([output]) for command, output in zip(COMMANDS, executables)})]

_generation = rule(implementation = _generation_impl, attrs = dict(FIX_EVAL,
    phases = attr.label_list(mandatory = True, providers = [FixValueInfo]),
    directory = attr.string(mandatory = True),
))

def _item(name, src, index):
    fix_select_index(name = name + "_selection", src = src, index = index, tags = _TAGS)
    fix_strict_encode(name = name, src = ":" + name + "_selection", tags = _TAGS)
    return ":" + name

def _command(name, executable, args, mounts, work, argv0):
    flatware_application(name = name + "_application", executable = executable, args = args, work = work, mounts = mounts, env = _ENV, argv0 = argv0, tags = _TAGS)
    flatware_work(name = name + "_work", src = ":" + name + "_application", fd = 3 + len(mounts), tags = _TAGS)
    return ":" + name + "_work"

def generation(name, previous, jobs = 0, memory = 16384):
    inputs = []
    for command in COMMANDS:
        prefix = name + "_" + command
        native.genrule(name = prefix + "_wasm", srcs = [":stage0"], outs = [name + "/" + command + "/guest.wasm"], cmd = "cp $(location :stage0)/commands/" + command + ".wasm $@", tags = _TAGS)
        flatware_directory(name = prefix + "_inputs", inputs = {":" + prefix + "_wasm": "guest.wasm", "//flatware:adapter_wasm": "adapter.wasm", "//flatware:control_wasm": "control.wasm"}, tags = _TAGS)
        inputs.append(":" + prefix + "_inputs")
    prepared = _phase(name + "_prepare", [":usr", ":shell_headers"] + [":" + previous + "_" + command + "_value" for command in COMMANDS] + inputs)
    usr = _item(name + "_usr", prepared, 0)
    shell = _item(name + "_shell", prepared, 1)
    mounts = {usr: "/usr"}
    compile_mounts = dict(mounts, **{shell: "/include"})
    tools = {command: _item(name + "_" + command + "_tool", prepared, 2 + i) for i, command in enumerate(COMMANDS)}
    merged = []
    translated = []
    compiled = []
    linked = []
    outputs = []
    for number, command in enumerate(COMMANDS):
        prefix = name + "_" + command
        work = _item(prefix + "_input", prepared, 5 + number)
        merged.append(_command(prefix + "_merge", tools["wasm-merge"], ["/work/adapter.wasm", "wasi_snapshot_preview1", "/work/guest.wasm", "flatware_guest", "/work/control.wasm", "flatware_control", "--all-features", "--disable-gc", "--rename-export-conflicts", "--no-resource-imports", "-o", "/work/module.wasm"], mounts, work, "/usr/bin/wasm-merge"))
        work = _item(prefix + "_merged", ":" + name + "_merge_eval", number)
        translated.append(_command(prefix + "_translate", tools["wasm2c"], ["-n", "module", "--num-outputs=256", "/work/module.wasm", "-o", "/work/module.c"], mounts, work, "/usr/bin/wasm2c"))
        work = _item(prefix + "_translated", ":" + name + "_translate_eval", number)
        objects = {}
        for i in range(257):
            application = prefix + "_compile_%d" % i
            source = "/work/module_%d.c" % i if i < 256 else "/include/wasm-rt.c"
            directory = _command(application, tools["clang"], _FLAGS + ["-I/include", "-I/work", "-c", source, "-o", "/work/object.o"], compile_mounts, work, "/usr/bin/clang")
            flatware_file(name = application + "_object", src = directory, path = "object.o", tags = _TAGS)
            index = len(compiled)
            compiled.append(":" + application + "_object")
            target = _item(prefix + "_object_%d" % i, ":" + name + "_compile_eval", index)
            objects[target] = "object_%d.o" % i
        flatware_files(name = prefix + "_objects", files = objects, tags = _TAGS)
        work = _command(prefix + "_linked", tools["clang"], _FLAGS + ["-fuse-ld=/usr/bin/ld.lld", "-Wl,--build-id=none", "-Wl,--threads=1", "-Wl,-T,/include/memmap.ld", "-o", "/work/" + command + ".elf"] + ["/work/" + path for path in objects.values()] + ["/include/libfixshell.a"], compile_mounts, ":" + prefix + "_objects", "/usr/bin/clang")
        flatware_file(name = prefix + "_executable", src = work, path = command + ".elf", tags = _TAGS)
        linked.append(":" + prefix + "_executable")
        _item(prefix + "_value", ":" + name + "_result", number)
        native.filegroup(name = prefix + "_link", srcs = [":" + name + "_result"], output_group = command, tags = _TAGS)
        outputs.append(":" + prefix + "_link")
    for phase, requests in [("merge", merged), ("translate", translated), ("compile", compiled), ("link", linked)]:
        _phase(name + "_" + phase, requests)
    settings = {option if type(value) == "int" else option + "_setting": value for option, value in [("jobs", jobs), ("memory", memory)]}
    _generation(name = name + "_result", phases = [":" + name + "_" + phase + "_requests" for phase in ["prepare", "merge", "translate", "compile", "link"]], directory = name, tags = _TAGS, **settings)
    stage(name = name, srcs = outputs + [":stage0", "//fix/shell:shell", "//fix/shell:wasm_rt", "//fix/shell:headers", "//fix/shell:memmap", "//flatware:adapter_wasm", "//flatware:control_wasm"], tags = _TAGS)

def _verify_impl(ctx):
    script = ctx.actions.declare_file(ctx.label.name + ".sh")
    comparisons = ['cmp "$root/%s" "$root/%s"' % (left.short_path, right.short_path) for left, right in zip(ctx.files.left, ctx.files.right)]
    ctx.actions.write(script, '#!/usr/bin/env bash\nset -euo pipefail\nroot="${RUNFILES_DIR:-$0.runfiles}/%s"\n' % ctx.workspace_name + "\n".join(comparisons) + "\n", is_executable = True)
    return [DefaultInfo(executable = script, runfiles = ctx.runfiles(files = ctx.files.left + ctx.files.right))]

_verify = rule(implementation = _verify_impl, executable = True, cfg = mode, attrs = dict(ALLOW_TRANSITION, left = attr.label_list(), right = attr.label_list()))

def verify(name, left, right):
    _verify(name = name, left = [":" + left + "_" + command + "_link" for command in COMMANDS], right = [":" + right + "_" + command + "_link" for command in COMMANDS], tags = ["manual"])
