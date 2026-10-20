"""Fix graphs are plumbing commands; fix_eval is their evaluation boundary."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")

FixValueInfo = provider(fields = ["expression", "nodes", "files", "stored", "inputs", "repositories"])
FixResultInfo = provider(fields = ["handle", "repository"])
FixToolchainInfo = provider(fields = ["repository", "trust", "handles"])

def _toolchain(ctx):
    return [FixToolchainInfo(repository = ctx.file.repository, trust = ctx.file.trust, handles = ctx.file.handles)]

fix_toolchain = rule(implementation = _toolchain, attrs = {
    "repository": attr.label(mandatory = True, allow_single_file = True),
    "trust": attr.label(mandatory = True, allow_single_file = True),
    "handles": attr.label(mandatory = True, allow_single_file = True),
})

def _key(ctx):
    return str(ctx.label)

def _value(target):
    return target[FixValueInfo].expression

def _path(file):
    return '"$execroot"/' + shell.quote(file.path)

def _graph(ctx, expression, dependencies = [], files = [], imports = []):
    values = [target[FixValueInfo] for target in dependencies]
    return [FixValueInfo(
        expression = {"node": _key(ctx)},
        nodes = depset([(_key(ctx), json.encode(expression))], transitive = [value.nodes for value in values], order = "postorder"),
        files = depset(imports, transitive = [value.files for value in values]),
        stored = depset(transitive = [value.stored for value in values]),
        inputs = depset(files, transitive = [value.inputs for value in values]),
        repositories = depset(transitive = [value.repositories for value in values]),
    ), DefaultInfo(files = depset())]

def _blob(ctx):
    if ctx.file.src:
        return _graph(ctx, {"binding": "input:" + _key(ctx)}, files = [ctx.file.src], imports = [("input:" + _key(ctx), ctx.file.src)])
    expression = ctx.attr.value if ctx.attr.format == "string" else {ctx.attr.format: int(ctx.attr.value)}
    return _graph(ctx, expression)

fix_blob = rule(implementation = _blob, attrs = {
    "src": attr.label(allow_single_file = True),
    "value": attr.string(),
    "format": attr.string(default = "string", values = ["string", "u8", "u16", "u32", "u64"]),
})

def _binding(ctx):
    return _graph(ctx, {"binding": ctx.attr.value})

fix_binding = rule(implementation = _binding, attrs = {"value": attr.string(mandatory = True)})

def _tree(ctx):
    return _graph(ctx, [_value(child) for child in ctx.attr.children], ctx.attr.children)

fix_tree = rule(implementation = _tree, attrs = {"children": attr.label_list(providers = [FixValueInfo])})

def _unary(ctx):
    value = _value(ctx.attr.src)
    return _graph(ctx, {ctx.attr.operation: value}, [ctx.attr.src])

_constructor = rule(implementation = _unary, attrs = {
    "src": attr.label(mandatory = True, providers = [FixValueInfo]),
    "operation": attr.string(mandatory = True),
})

def fix_ref(name, src, **kwargs):
    _constructor(name = name, src = src, operation = "create-ref", **kwargs)

def fix_identification_thunk(name, src, **kwargs):
    _constructor(name = name, src = src, operation = "create-identification-thunk", **kwargs)

def fix_application_thunk(name, src, **kwargs):
    _constructor(name = name, src = src, operation = "create-application-thunk", **kwargs)

def fix_selection_thunk(name, src, **kwargs):
    _constructor(name = name, src = src, operation = "create-selection-thunk", **kwargs)

def _select(ctx):
    indices = [{"u64": ctx.attr.begin}]
    if ctx.attr.end >= 0:
        indices.append({"u64": ctx.attr.end})
    if ctx.attr.begin < 0 or (ctx.attr.end >= 0 and ctx.attr.begin > ctx.attr.end):
        fail("expected a nonnegative index or an ordered range")
    expression = {"create-selection-thunk": [_value(ctx.attr.src)] + indices}
    return _graph(ctx, expression, [ctx.attr.src])

_selection = rule(implementation = _select, attrs = {
    "src": attr.label(mandatory = True, providers = [FixValueInfo]),
    "begin": attr.int(mandatory = True),
    "end": attr.int(default = -1),
})

def fix_select_index(name, src, index, **kwargs):
    _selection(name = name, src = src, begin = index, **kwargs)

def fix_select_range(name, src, begin, end, **kwargs):
    if end < 0:
        fail("expected a nonnegative range endpoint")
    _selection(name = name, src = src, begin = begin, end = end, **kwargs)

def fix_strict_encode(name, src, **kwargs):
    _constructor(name = name, src = src, operation = "create-strict-encode", **kwargs)

def fix_shallow_encode(name, src, **kwargs):
    _constructor(name = name, src = src, operation = "create-shallow-encode", **kwargs)

def fix_action(ctx, repositories, inputs, outputs, command, mnemonic, jobs = 1, memory = 16384, stored = []):
    """Run plumbing against the persistent repository with declared inputs."""
    toolchain = ctx.attr.toolchain[FixToolchainInfo]
    script = ctx.actions.declare_file(ctx.label.name + "/" + mnemonic + ".sh")
    config = ctx.actions.declare_file(ctx.label.name + "/" + mnemonic + ".json")
    ctx.actions.write(script, "set -euo pipefail\nshopt -s inherit_errexit\n" + command + "\n")
    ctx.actions.write(config, json.encode({
        "cache": ctx.attr._cache[BuildSettingInfo].value,
        "fix": ctx.executable._fix.path,
        "trust": toolchain.trust.path,
        "toolchain": toolchain.handles.path,
        "repository": toolchain.repository.path,
        "repositories": [(archive.path, handle.path) for archive, handle in repositories],
        "script": script.path,
        "stored": [handle.path for handle in stored],
    }))
    ctx.actions.run(
        executable = ctx.executable._action,
        arguments = [config.path],
        inputs = depset([file for pair in repositories for file in pair] + [toolchain.repository, toolchain.trust, toolchain.handles, config, script], transitive = [inputs]),
        tools = [ctx.attr._fix[DefaultInfo].files_to_run],
        outputs = outputs,
        mnemonic = mnemonic,
        use_default_shell_env = True,
        execution_requirements = {
            "no-sandbox": "1",
            "no-remote-exec": "1",
            "resources:fix:1": "",
            "resources:cpu:%d" % max(1, jobs): "",
            "resources:memory:%d" % memory: "",
        },
    )

FIX_TOOL = {
    "toolchain": attr.label(default = "//bootstrap:repository_archive", providers = [FixToolchainInfo]),
    "_fix": attr.label(default = "//fix", executable = True, cfg = "target"),
    "_ndjson": attr.label(default = "//build:ndjson", executable = True, cfg = "exec"),
    "_action": attr.label(default = "//build:fix-action", executable = True, cfg = "exec"),
    "_cache": attr.label(default = "//build:fix-cache", providers = [BuildSettingInfo]),
}

def fix_graph_description(ctx, value, name):
    source = ctx.actions.declare_file(ctx.label.name + "/" + name + ".jsonl")
    manifest = ctx.actions.declare_file(ctx.label.name + "/" + name + ".nodes.json")
    ctx.actions.write(manifest, json.encode({"nodes": [(key, json.decode(expression)) for key, expression in value.nodes.to_list()], "result": value.expression}))
    ctx.actions.run(executable = ctx.executable._ndjson, arguments = [manifest.path, source.path], inputs = [manifest], outputs = [source], mnemonic = "FixGraphDescription")
    return source

def fix_graph_commands(ctx, value, source, handle, bindings = None):
    commands = [': > names; : > paths; : > bindings']
    for key, file in value.files.to_list():
        commands.append('printf "%%s\\0" %s >> names; printf "%%s\\0" %s >> paths' % (shell.quote(key), _path(file)))
    commands += [
        '"$FIX" create-blob --files-from paths > handles',
        'while IFS= read -r -d "" name && IFS= read -r handle <&3; do printf "%s\\0%s\\0" "$name" "$handle" >> bindings; done < names 3< handles',
    ]
    for key, stored_handle in value.stored.to_list():
        commands.append('printf "%%s\\0%%s\\0" %s "$(cat %s)" >> bindings' % (shell.quote(key), _path(stored_handle)))
    if bindings:
        commands.append('cat %s >> bindings' % shell.quote(bindings))
    commands += [
        'environment=$("$FIX" create-tree --bindings - < bindings)',
        '"$FIX" read-tree "$FIX_BINDINGS" > environment.children',
        '"$FIX" read-tree "$environment" >> environment.children',
        'environment=$("$FIX" create-tree - < environment.children)',
        '"$FIX" json-import %s "$environment" > %s' % (_path(source), handle),
    ]
    return commands

def _materialize(ctx, value):
    graph = ctx.actions.declare_file(ctx.label.name + "/graph.tar")
    graph_handle = ctx.actions.declare_file(ctx.label.name + "/graph.handle")
    source = fix_graph_description(ctx, value, "graph")
    commands = fix_graph_commands(ctx, value, source, _path(graph_handle)) + [
        '"$FIX" export "$(cat %s)" %s' % (_path(graph_handle), _path(graph)),
    ]
    fix_action(ctx, value.repositories.to_list(), depset([source], transitive = [value.inputs]), [graph, graph_handle], "\n".join(commands), "FixGraph", stored = [graph_handle])
    return graph, graph_handle

def _stored(ctx, repository, handle):
    key = _key(ctx)
    return [FixValueInfo(
        expression = {"binding": "input:" + key},
        nodes = depset(order = "postorder"),
        files = depset(),
        stored = depset([("input:" + key, handle)]),
        inputs = depset([handle]),
        repositories = depset([(repository, handle)]),
    ), DefaultInfo(files = depset([handle, repository]))]

def fix_import(ctx, value):
    repository, handle = _materialize(ctx, value)
    return _stored(ctx, repository, handle)

def _build(ctx):
    return fix_import(ctx, ctx.attr.src[FixValueInfo])

fix_build = rule(implementation = _build, attrs = dict(FIX_TOOL, src = attr.label(mandatory = True, providers = [FixValueInfo])))

def fix_budgets(ctx):
    jobs = ctx.attr.jobs_setting[BuildSettingInfo].value if ctx.attr.jobs_setting else ctx.attr.jobs
    memory = ctx.attr.memory_setting[BuildSettingInfo].value if ctx.attr.memory_setting else ctx.attr.memory
    if jobs < 0:
        fail("jobs must be nonnegative (0 uses available parallelism)")
    jobs = jobs or ctx.attr._jobs[BuildSettingInfo].value
    if memory == 0:
        memory = 16384
        for _ in range(32):
            if memory >= jobs * 1024:
                break
            memory *= 2
    if memory < 16 or memory & (memory - 1):
        fail("memory must be 0 (automatic) or a power of two, at least 16 MiB")
    return jobs, memory

def _eval(ctx):
    graph, graph_handle = _materialize(ctx, ctx.attr.src[FixValueInfo])
    repository = ctx.actions.declare_file(ctx.label.name + "/result.tar")
    handle = ctx.actions.declare_file(ctx.label.name + "/result.handle")
    jobs, memory = fix_budgets(ctx)
    options = " --memoize --memory %d -j %d" % (memory, jobs)
    fix_action(ctx, [(graph, graph_handle)], depset([graph_handle]), [repository, handle],
               '"$FIX" eval%s "$(cat %s)" > %s\n"$FIX" export "$(cat %s)" %s' % (options, _path(graph_handle), _path(handle), _path(handle), _path(repository)), "FixEval", jobs = jobs, memory = memory, stored = [handle])
    return _stored(ctx, repository, handle) + [FixResultInfo(handle = handle, repository = repository), OutputGroupInfo(graph = depset([graph, graph_handle]))]

FIX_EVAL = dict(FIX_TOOL,
    jobs = attr.int(),
    _jobs = attr.label(default = "//build:fix-jobs"),
    memory = attr.int(default = 16384),
    jobs_setting = attr.label(providers = [BuildSettingInfo]),
    memory_setting = attr.label(providers = [BuildSettingInfo]),
)

_eval_rule = rule(implementation = _eval, attrs = dict(FIX_EVAL, src = attr.label(mandatory = True, providers = [FixValueInfo])))

def fix_eval(name, src, jobs = 0, memory = 16384, **kwargs):
    """Evaluate with integer budgets or labels of integer build settings."""
    settings = {}
    for option, value in [("jobs", jobs), ("memory", memory)]:
        settings[option if type(value) == "int" else option + "_setting"] = value
    _eval_rule(name = name, src = src, **dict(settings, **kwargs))

def _read_blob(ctx):
    if FixResultInfo in ctx.attr.src:
        result = ctx.attr.src[FixResultInfo]
        repository, handle = result.repository, result.handle
    else:
        repository, handle = _materialize(ctx, ctx.attr.src[FixValueInfo])
    output = ctx.actions.declare_file(ctx.attr.out)
    command = 'value=$(cat %s)\n' % _path(handle)
    if ctx.attr.index >= 0:
        command += 'mapfile -t children < <("$FIX" read-tree "$value")\nvalue=${children[%d]}\n' % ctx.attr.index
    command += '"$FIX" read-blob "$value" > %s' % _path(output)
    fix_action(ctx, [(repository, handle)], depset([handle]), [output], command, "FixReadBlob")
    return [DefaultInfo(files = depset([output]))]

fix_read_blob = rule(implementation = _read_blob, attrs = dict(FIX_TOOL,
    src = attr.label(mandatory = True, providers = [FixValueInfo]),
    out = attr.string(mandatory = True),
    index = attr.int(default = -1),
))

fix_graph = _graph
fix_value = _value
fix_path = _path
fix_key = _key
fix_stored = _stored
