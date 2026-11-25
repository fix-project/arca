import fix
import flatware


@fix.do
async def arithmetic(add, left, right):
    temporary = b"used only in this stage"
    assert len(temporary) > 0
    total = await fix.call(add, left, right)
    for candidate in (left, right):
        number = int(candidate)
        if number < 3:
            continue
        selected = candidate
        break
    else:
        raise ValueError("no candidate")
    a, b = await fix.parallel(fix.call(add, total, left), fix.call(add, total, selected))
    count = 2
    while count:
        count -= 1
    if int(a) < int(b):
        return fix.Tree([a, b])
    return fix.Tree([b, a])


def resolve(path, base, changes):
    assert path == (b"/work", b"a")
    assert base is None
    assert len(changes) == 2
    return changes[0]


@fix.do
async def control_flow(add, total):
    iterator = iter((1, 2))
    assert next(iterator) == 1
    marker = {"number": 1 << 80, "values": (None, 1.5, b"bytes")}
    alias = marker
    native = (bytearray(b"data"), {1, 2}, frozenset({3}), 2 + 3j, True)
    for step in range(4):
        if step == 0:
            continue
        match step:
            case 1:
                total = await fix.call(add, total, 2)
            case 2:
                total = await fix.call(add, total, 3)
            case _:
                break
    else:
        raise AssertionError("expected break")
    while int(total) < 10:
        total = await fix.call(add, total, 1)
    assert native == (bytearray(b"data"), {1, 2}, frozenset({3}), 2 + 3j, True)
    match marker:
        case {"number": number, "values": (None, 1.5, b"bytes")} if number == 1 << 80:
            assert marker is alias
        case _:
            raise AssertionError("captured state changed")
    return await fix.call(add, total, 1)


@fix.do
async def nonserializable(add, value):
    iterator = iter((value,))
    await fix.call(add, value, value)
    return next(iterator)


@flatware.do(resolve=resolve)
async def build(unix, copy, cat):
    for source, target in (("input", "intermediate"), ("intermediate", "prepared")):
        prepared = await unix.run(copy, ["probe", "copy", f"/work/{source}", f"/work/{target}"])
        assert prepared.exit_status == 0
    unix.remove("/work/intermediate")
    first, second, third = await unix.parallel(
        unix.run(copy, ["probe", "copy", "/work/prepared", "/work/a"]),
        unix.run(copy, ["probe", "copy", "/work/other", "/work/a"]),
        unix.run(copy, ["probe", "copy", "/work/other", "/work/b"]),
    )
    assert [item.exit_status for item in (first, second, third)] == [0, 0, 0]
    result = await unix.run(cat, ["cat", "/work/a", "/work/b"])
    assert result.exit_status == 0
    return result.stdout


@flatware.do
async def conflict(unix, copy):
    await unix.parallel(
        unix.run(copy, ["probe", "copy", "/work/input", "/work/conflict"]),
        unix.run(copy, ["probe", "copy", "/work/other", "/work/conflict"]),
    )
    return fix.Blob(b"unreachable")


@flatware.do
async def edit_environment(unix, probe):
    assert unix.getenv("GREETING", "missing") == "missing"
    unix.setenv("GREETING", "wrong")
    unix.setenv("GREETING", "hello")
    unix.mount("/work", fix.Tree())
    unix.mount("/scratch", fix.Tree())
    deferred_directory = fix.Tree([fix.Tree([
        flatware.file("notes", b"deferred mount"),
    ])]).reference().select(0).strict()
    unix.mount("/nested/deferred", deferred_directory)
    assert unix.listdir() == (b"work", b"scratch", b"nested")
    assert unix.listdir("/nested") == (b"deferred",)
    unix.mount("/", fix.Tree([
        flatware.file("local"),
        flatware.dirent("work", fix.Tree()),
    ]))
    assert unix.listdir("/") == (b"local", b"work", b"scratch", b"nested")
    unix.unmount("/")
    unix.mkdir("/scratch/nested")
    unix.write_file("/scratch/nested/notes", b"notes")
    unix.write_file("/work/input.txt", b"old")
    deferred_file = fix.Tree([b"original"]).reference().select(0).strict()
    unix.write_file("/work/input.txt", deferred_file)
    assert isinstance(unix.read_file("/work/input.txt"), fix.Encode)
    result = await unix.run(probe, ["probe", "argument"], stdin=b"through Flatware")
    assert result.exit_status == 23
    assert unix.getenv("GREETING") == "hello"
    assert bytes(unix.read_file("/work/renamed.txt")) == b"original"
    assert bytes(unix.read_file("/nested/deferred/notes")) == b"deferred mount"
    assert unix.listdir("/scratch/nested") == (b"notes",)
    unix.remove("/scratch/nested/notes")
    unix.remove("/scratch/nested")
    assert bytes(unix.unmount("/scratch")[0]) == b"/scratch"
    unix.unsetenv("GREETING")
    unix.setenv("FINAL", "done")
    return result.stdout


@flatware.do
async def deferred_contents(unix, cat):
    contents = fix.Tree([b"mounted ", b"written ", b"stdin"]).reference()
    directory = fix.Tree([fix.Tree([flatware.file("input", contents.select(0))])])
    unix.mount("/deferred", directory.reference().select(0))
    unix.mount("/work", ())
    unix.write_file("/work/output", b"old")
    unix.write_file("/work/output", contents.select(1))
    result = await unix.run(cat, ["cat", "/deferred/input", "/work/output", "-"],
                            stdin=contents.select(2), metadata=fix.Blob(1 << 30))
    assert isinstance(result.descriptors, fix.Tree), str(result.descriptors)
    assert result.exit_status == 0, str(result.stderr)
    output = unix.read_file("/work/output")
    assert isinstance(output, fix.Reference)
    assert output.data_type == "blob"
    return result.stdout, output.select(1, 4).strict()


if __name__ == "__main__":
    add, copy, cat, left, right = fix.argv
    environment = flatware.Environment(env={"LANG": "C"})
    environment.mount("/work", fix.Tree([
        flatware.file("input", b"left"), flatware.file("other", b"right"),
    ]))
    fix.exit([
        arithmetic(add, left, right).strict(),
        build(environment, copy, cat).strict(),
        conflict(environment, copy).strict(),
        edit_environment(flatware.Environment(), copy).strict(),
        control_flow(add, left).strict(),
        nonserializable(add, left).strict(),
        deferred_contents(flatware.Environment(), cat).strict(),
    ])
