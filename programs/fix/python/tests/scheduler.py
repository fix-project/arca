import fix
from fix.scheduler import Scheduler, run


class Policy(Scheduler):
    def __init__(self):
        self.advice = {}
        self.loaded = 0

    async def load(self, value):
        self.loaded += 1
        return await super().load(value)

    async def apply(self, combination):
        if len(combination) == 3 and combination[2] == fix.Blob(b"fail"):
            raise ValueError("policy rejected the application")
        result = await super().apply(combination)
        if result == fix.Blob(b"hello, world"):
            assert self.loaded > 0
            expected = {
                b"fix::output-size": (42).to_bytes(8, "little"),
                b"fix::parallel": (2).to_bytes(8, "little"),
            }
            if len(combination) == 3:
                assert bytes(combination[2]) == b"python"
                expected[b"fix::title"] = "café".encode("utf-8")
                expected[b"fix::canonicalize"] = b"\x01"
                expected[b"fix::log"] = "debug / 42 café\n".encode("utf-8")
            assert self.advice == expected
            return fix.Blob(b"scheduled by Python")
        return result

    def on_advice(self, target, key, payload):
        assert isinstance(target, fix.Tree)
        self.advice[bytes(key)] = bytes(payload)


print("Python scheduler started")
fix.exit(run(Policy()))
