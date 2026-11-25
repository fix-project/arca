"""Async policies for the Fix evaluator."""

from fix import _unwrap, _wrap
from _fix import run_scheduler as _run


class _Default:
    def __init__(self, operation, value):
        self.request = (operation, _unwrap(value))

    def __await__(self):
        return _wrap((yield self.request))


class Scheduler:
    async def eval(self, value):
        return await _Default("eval", value)

    async def think(self, thunk):
        return await _Default("think", thunk)

    async def force(self, thunk):
        return await _Default("force", thunk)

    async def apply(self, combination):
        return await _Default("apply", combination)

    async def select(self, combination):
        return await _Default("select", combination)

    async def load(self, value):
        return await _Default("load", value)

    def on_advice(self, target, key, payload):
        pass


def _invoke(policy, operation, value):
    result = yield from getattr(policy, operation)(_wrap(value)).__await__()
    return _unwrap(result)


def _on_advice(policy, target, key, payload):
    policy.on_advice(_wrap(target), _wrap(key), _wrap(payload))


def run(policy):
    """Evaluate the current scheduler invocation using policy."""
    if not isinstance(policy, Scheduler):
        raise TypeError("expected a Scheduler instance")
    return _wrap(_run(policy, _invoke, _on_advice))
