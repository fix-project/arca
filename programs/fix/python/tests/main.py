import fix
import flatware
from itertools import chain


prefix = b"fresh:"


@fix.function
def greet(value: str):
    return prefix.decode("utf-8") + value


@fix.function
def negative(value: int) -> bool:
    return value < 0


@fix.function
def containers(data: dict[str, list[int]], pair: tuple[int, str]) -> tuple[list[int], dict[str, str]]:
    count, text = pair
    assert data == {"numbers": [3, -1]}
    return [*data["numbers"], count], {"text": text}


if __name__ == "__main__":
    program, value, count = fix.arity(3)
    if remaining := int(count):
        fix.exit(fix.call(fix.callable(), program, value, remaining - 1).strict())
    args = chain(["cat"], [])
    command = flatware.run(program, args, stdin=bytes(value))
    assert isinstance(command, fix.Thunk)
    result = flatware.Result(command.strict().eval())
    assert result.exit_status == 0
    assert bytes(result.stderr) == b""
    prefix = b"old:"
    assert isinstance(greet, fix.Tree)
    converted = fix.call(containers, {"numbers": [3, -1]}, (7, "hello")).strict()
    inferred = {"inferred": ("nested", [1, False])}
    fix.exit([result.stdout, greet, negative, containers, converted, inferred])
