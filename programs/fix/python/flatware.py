"""Build Flatware applications and read their results."""

import fix
from fix import Blob, Tree
from _fix import stat as _stat


def _blob(value):
    if isinstance(value, Blob):
        return value
    if isinstance(value, str):
        value = value.encode("utf-8")
    return Blob(value)


def stat(filetype, size, *, device=0, links=1, atime=0, mtime=0, ctime=0):
    return fix._wrap(_stat(device, filetype, links, size, atime, mtime, ctime))


def descriptor(name, stat, contents):
    return Tree([_blob(name), stat, contents])


def _file_contents(contents):
    if isinstance(contents, fix.Encode):
        return contents
    if isinstance(contents, fix.Reference):
        if contents.data_type != "blob":
            raise TypeError("file contents must refer to a Blob")
        return contents
    return _blob(contents)


def _size(value):
    return 0 if isinstance(value, fix.Encode) else len(value)


def file(name, contents=b""):
    """Build a regular file entry."""
    return dirent(name, contents, filetype=4)


def dirent(name, contents, *, filetype=None):
    """Wrap directory or file contents with a name and native metadata."""
    if filetype is None:
        if isinstance(contents, fix.Encode):
            raise TypeError("specify filetype for encoded contents")
        is_tree = isinstance(contents, Tree) or (
            isinstance(contents, fix.Reference) and contents.data_type == "tree"
        )
        filetype = 3 if is_tree else 4
    if filetype == 3:
        if isinstance(contents, fix.Reference):
            if contents.data_type != "tree":
                raise TypeError("directory entries must refer to a Tree")
        elif not isinstance(contents, (Tree, fix.Encode)):
            contents = Tree(contents)
    elif filetype == 4:
        contents = _file_contents(contents)
    else:
        raise ValueError("expected a directory (3) or regular file (4)")
    return descriptor(name, stat(filetype, _size(contents)), contents)


def _stream(name, contents=b""):
    return descriptor(name, Tree(), _blob(contents))


def _environment(env):
    environment = () if env is None else env.items()
    return Tree(_blob(f"{name}={value}") for name, value in environment)


def _u64(value):
    return Blob(value.to_bytes(8, "little"))


def _replace_at(tree, index, value):
    if value is None:
        return Tree(child for i, child in enumerate(tree) if i != index)
    if index == len(tree):
        return Tree([*tree, value])
    return Tree(value if i == index else child for i, child in enumerate(tree))


def _variable_name(name):
    if not isinstance(name, str):
        raise TypeError("environment variable name must be a string")
    if not name or "=" in name or "\x00" in name:
        raise ValueError("invalid environment variable name")
    return name.encode("utf-8")


def _path(path):
    if isinstance(path, str):
        path = path.encode("utf-8")
    if not isinstance(path, bytes):
        raise TypeError("path must be a string or bytes")
    if not path.startswith(b"/") or b"\x00" in path:
        raise ValueError("expected an absolute Flatware path")
    parts = []
    for part in path.split(b"/"):
        if part == b"..":
            if parts:
                parts.pop()
        elif part and part != b".":
            parts.append(part)
    return tuple(parts)


def _filetype(entry):
    if not isinstance(entry[1], Tree):
        raise ValueError("file metadata must be materialized")
    return bytes(entry[1][1])


def _children(entry):
    if _filetype(entry) != b"\x03":
        raise NotADirectoryError(bytes(entry[0]))
    if not _native_directory(entry):
        raise ValueError("directory must be materialized in the native Flatware format")
    return entry[2]


def _regular_file(entry, path):
    if entry is None:
        raise FileNotFoundError(path)
    filetype = _filetype(entry)
    if filetype == b"\x03":
        raise IsADirectoryError(path)
    if filetype != b"\x04" or entry[1][0] != _u64(0):
        raise ValueError("expected a native regular file")


def _created(name, filetype, contents, timestamp):
    time = int.from_bytes(bytes(timestamp), "little")
    return descriptor(name, stat(filetype, _size(contents), atime=time, mtime=time, ctime=time),
                      contents)


def _updated(entry, contents, timestamp=None):
    fields = list(entry[1])
    fields[3] = _u64(_size(contents))
    if timestamp is not None:
        fields[5] = fields[6] = timestamp
    return descriptor(entry[0], Tree(fields), contents)


def _application(program, argv, env, stdin, preopens, seed, timestamp, metadata):
    return fix.call(
        program,
        Tree(map(_blob, argv)),
        env,
        Tree([_stream("stdin", stdin), _stream("stdout"), _stream("stderr"), *preopens]),
        seed,
        timestamp,
        metadata=metadata,
    )


def run(program, argv=(), *, env=None, stdin=b"", preopens=(), seed=0, timestamp=0, metadata=None):
    """Construct an application Thunk for a Flatware program."""
    return _application(program, argv, _environment(env), stdin, preopens,
                        _u64(seed), _u64(timestamp), metadata)


call = run


class Result:
    """Read the descriptors and status from an evaluated Flatware result."""

    def __init__(self, tree):
        self.descriptors, status = tree
        self.exit_status = int.from_bytes(bytes(status), "little")

    @property
    def stdout(self):
        return self.descriptors[1][2]

    @property
    def stderr(self):
        return self.descriptors[2][2]


class _Command:
    def __init__(self, program, argv, stdin, metadata):
        self.program = program
        self.argv = tuple(argv)
        self.stdin = stdin
        self.metadata = metadata


class _Commands:
    def __init__(self, commands):
        self.commands = commands


class Environment:
    """Environment variables, preopens, and deterministic program inputs."""

    def __init__(self, *, env=None, preopens=(), seed=0, timestamp=0):
        self.value = Tree([_environment(env), Tree(preopens), _u64(seed), _u64(timestamp)])
        self._resolve = None

    @classmethod
    def from_value(cls, value):
        """Restore the environment returned by a Flatware workflow."""
        environment = object.__new__(cls)
        environment.value = value
        environment._resolve = None
        return environment

    @property
    def preopens(self):
        return self.value[1]

    def _replace(self, index, value):
        self.value = _replace_at(self.value, index, value)

    def getenv(self, name, default=None):
        """Look up a UTF-8 environment variable."""
        key = _variable_name(name)
        for entry in self.value[0]:
            prefix, _, value = bytes(entry).partition(b"=")
            if prefix == key:
                return value.decode("utf-8")
        return default

    def setenv(self, name, value):
        """Set an environment variable for subsequent commands."""
        if not isinstance(value, str):
            raise TypeError("environment variable value must be a string")
        if "\x00" in value:
            raise ValueError("invalid environment variable value")
        key = _variable_name(name)
        self._edit_variable(key, Blob(key + b"=" + value.encode("utf-8")))

    def unsetenv(self, name):
        """Remove an environment variable if present."""
        self._edit_variable(_variable_name(name), None)

    def _edit_variable(self, key, replacement):
        entries = []
        for entry in self.value[0]:
            if bytes(entry).partition(b"=")[0] != key:
                entries.append(entry)
            elif replacement is not None:
                entries.append(replacement)
                replacement = None
        if replacement is not None:
            entries.append(replacement)
        self._replace(0, Tree(entries))

    def _mounts(self):
        for index, entry in enumerate(self.preopens):
            name = bytes(entry[0])
            if name.startswith(b"/"):
                yield _path(name), index, entry

    def mount(self, path, contents):
        """Mount directory contents at an absolute path."""
        parts = _path(path)
        if any(parts == mounted for mounted, _, _ in self._mounts()):
            raise FileExistsError(path)
        mounted = dirent(b"/" + b"/".join(parts), contents, filetype=3)
        self._replace(1, Tree([*self.preopens, mounted]))

    def unmount(self, path):
        """Remove a preopen and return its descriptor."""
        parts = _path(path)
        for mounted, index, entry in self._mounts():
            if parts == mounted:
                self._replace(1, _replace_at(self.preopens, index, None))
                return entry
        raise FileNotFoundError(path)

    def _lookup(self, path):
        parts = _path(path)
        mount = max(
            (root for root in self._mounts() if parts[:len(root[0])] == root[0]),
            key=lambda root: len(root[0]),
            default=None,
        )
        if mount is None:
            raise FileNotFoundError(path)
        prefix, index, entry = mount
        parents = []
        for name in parts[len(prefix):]:
            if entry is None:
                raise FileNotFoundError(path)
            children = _children(entry)
            child_index, child = len(children), None
            for i, (child_name, descriptor) in enumerate(_named_entries(children)):
                if child_name == name:
                    child_index, child = i, descriptor
            parents.append((entry, name, child_index))
            entry = child
        return index, parents, entry

    def listdir(self, path="/"):
        """Return the byte names of directory entries and child mounts."""
        parts = _path(path)
        mounts = (
            mounted[len(parts)] for mounted, _, _ in self._mounts()
            if len(mounted) > len(parts) and mounted[:len(parts)] == parts
        )
        mount_names = dict.fromkeys(mounts)
        try:
            _, _, entry = self._lookup(path)
        except FileNotFoundError:
            entry = None
        if entry is not None:
            names = dict.fromkeys(name for name, _ in _named_entries(_children(entry)))
            names.update(mount_names)
            return tuple(names)
        if not mount_names:
            raise FileNotFoundError(path)
        return tuple(mount_names)

    def read_file(self, path):
        """Return the Fix value containing the regular file's contents."""
        _, _, entry = self._lookup(path)
        _regular_file(entry, path)
        return entry[2]

    def _edit_path(self, path, transform):
        index, parents, entry = self._lookup(path)
        if not parents:
            raise PermissionError("use mount or unmount to change a preopen")
        timestamp = self.value[3]
        replacement = transform(parents[-1][1], entry, timestamp)
        for directory, _, child_index in reversed(parents):
            children = directory[2]
            membership_changed = child_index == len(children) or replacement is None
            replacement = _updated(directory, _replace_at(children, child_index, replacement),
                                   timestamp if membership_changed else None)
        self._replace(1, _replace_at(self.preopens, index, replacement))

    def write_file(self, path, contents):
        """Create or replace the contents of a regular file."""
        def write(name, entry, timestamp):
            data = _file_contents(contents)
            if entry is None:
                return _created(name, 4, data, timestamp)
            _regular_file(entry, path)
            return _updated(entry, data, timestamp)

        self._edit_path(path, write)

    def mkdir(self, path):
        """Create a native directory with an existing parent."""
        def create(name, entry, timestamp):
            if entry is not None:
                raise FileExistsError(path)
            return _created(name, 3, Tree(), timestamp)

        self._edit_path(path, create)

    def remove(self, path):
        """Remove a file or an empty native directory."""
        def delete(_name, entry, _timestamp):
            if entry is None:
                raise FileNotFoundError(path)
            if _filetype(entry) == b"\x03" and _children(entry):
                raise OSError("directory is not empty")
            return None

        self._edit_path(path, delete)

    def run(self, program, argv=(), *, stdin=b"", metadata=None):
        """Prepare a command for the next workflow await."""
        return _Command(program, argv, stdin, metadata)

    def parallel(self, *commands):
        """Fork commands from the current environment."""
        return _Commands(commands)

    def _prepare(self, operation):
        env, preopens, seed, timestamp = self.value

        def command(request):
            if not isinstance(request, _Command):
                raise TypeError("await expects a Flatware workflow command")
            return _application(request.program, request.argv, env, request.stdin,
                                preopens, seed, timestamp, request.metadata).strict()

        if isinstance(operation, _Commands):
            return Tree([Blob(), Tree(command(request) for request in operation.commands)])
        return Tree([Blob(b"one"), command(operation)])

    def _resume(self, reply):
        kind, payload = reply
        env, preopens, seed, timestamp = self.value
        if len(kind):
            result = Result(payload)
            preopens = result.descriptors[3:]
        else:
            result = tuple(Result(tree) for tree in payload)
            branches = [branch.descriptors[3:] for branch in result]
            merged = (
                _merge(base, [branch[index] for branch in branches], (bytes(base[0]),), self._resolve)
                for index, base in enumerate(preopens)
            )
            try:
                preopens = Tree(entry for entry in merged if entry is not None)
            except MergeConflict as error:
                path = Tree(Blob(component) for component in error.path)
                return None, Tree([Blob(b"Flatware.merge-conflict"), path, self.value])
        return Tree([env, preopens, seed, timestamp]), result


class MergeConflict(ValueError):
    def __init__(self, path):
        self.path = path
        super().__init__(f"conflicting Flatware changes at {path!r}")


def _changes(base, branches):
    changed = []
    for branch in branches:
        if branch != base and all(branch != other for other in changed):
            changed.append(branch)
    return changed


def _named_entries(entries):
    seen = set()
    for entry in entries:
        name = bytes(entry[0])
        if name in seen:
            raise ValueError(f"duplicate Flatware entry {name!r}")
        seen.add(name)
        yield name, entry


def _native_directory(descriptor):
    return (
        descriptor is not None
        and isinstance(descriptor[1], Tree)
        and isinstance(descriptor[2], Tree)
        and descriptor[1][0] == _u64(0)
        and descriptor[1][1] == Blob(b"\x03")
    )


def _merge(base, branches, path, resolve):
    changed = _changes(base, branches)
    if not changed:
        return base
    if len(changed) == 1:
        return changed[0]

    def conflict():
        if resolve is None:
            raise MergeConflict(path)
        entry = resolve(path, base, tuple(changed))
        if entry is not None and (
            not isinstance(entry, Tree) or len(entry) != 3 or bytes(entry[0]) != path[-1]
        ):
            raise TypeError("merge resolver must return a matching descriptor or None")
        return entry

    if not (all(map(_native_directory, changed)) and (base is None or _native_directory(base))):
        return conflict()

    initial = base[1] if base is not None else stat(3, 0)
    stats = [entry[1] for entry in changed]
    fields = list(initial)
    for index in (2, 4):
        values = _changes(initial[index], [attributes[index] for attributes in stats])
        if len(values) > 1:
            return conflict()
        if values:
            fields[index] = values[0]
    for index in (5, 6):
        fields[index] = max((attributes[index] for attributes in stats),
                            key=lambda value: int.from_bytes(bytes(value), "little"))

    original = {} if base is None else dict(_named_entries(base[2]))
    versions = [dict(_named_entries(entry[2])) for entry in changed]
    names = original.keys() | {name for version in versions for name in version}
    children = []
    for name in sorted(names):
        merged = _merge(original.get(name), [version.get(name) for version in versions],
                        (*path, name), resolve)
        if merged is not None:
            children.append(merged)
    fields[3] = _u64(len(children))
    return descriptor(changed[0][0], Tree(fields), Tree(children))


class _Workflow:
    def __init__(self, resolve):
        self.resolve = resolve

    def initial(self, args):
        if not args or not isinstance(args[0], Environment):
            raise TypeError("expected a Flatware Environment as the first argument")
        return args[0].value, args[1:]

    def _from_value(self, value):
        environment = Environment.from_value(value)
        environment._resolve = self.resolve
        return environment


def do(fn=None, *, resolve=None):
    """Compile a workflow with a shared Flatware environment."""
    from fix._do import compile_flow

    if resolve is not None and not callable(resolve):
        raise TypeError("Flatware merge resolver must be callable")

    def decorate(function):
        return compile_flow(function, _Workflow(resolve))

    return decorate if fn is None else decorate(fn)
