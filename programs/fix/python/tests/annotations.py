import fix


@fix.function
def entry(data: bytes, text: "str", count: int, reference: "fix.Reference",
          thunk: fix.Thunk, enabled: bool, disabled: bool, *more: int) -> str:
    assert data == b"raw bytes stored in a separate Blob"
    assert reference.data_type == "blob"
    assert isinstance(thunk, fix.Thunk)
    assert enabled is True
    assert disabled is False
    return text * (count + sum(more))


if __name__ == "__main__":
    fix.exit(entry)
