"""Compile async workflows into lifted Fix stages."""

import _ast as ast
import fix


class Parallel:
    def __init__(self, operations):
        self.operations = operations


def _dependency(operation):
    if isinstance(operation, fix.Thunk):
        return operation.strict()
    if isinstance(operation, fix.Encode):
        return operation
    if isinstance(operation, Parallel):
        return fix.Tree(_dependency(item) for item in operation.operations)
    raise TypeError("await expects a Fix Thunk, Encode, or parallel group")


class _Step:
    def __init__(self, adapter, successor):
        self.adapter = adapter
        self.successor = successor

    def context(self, state):
        return self.adapter._from_value(state)

    def resume(self, state, reply):
        if self.adapter is None:
            return state, reply
        return self.context(state)._resume(reply)

    def advance(self, state, operation, *values):
        for value in values:
            if not isinstance(value, fix.Value) or not fix._unwrap(value).is_value():
                raise TypeError("locals carried across await must be Fix values")
        if self.adapter is None:
            dependency = _dependency(operation)
        else:
            dependency = state._prepare(operation)
            state = state.value
        return fix.call(self.successor, state, dependency, *values)

    def finish(self, state, result):
        if self.adapter is None:
            return result
        return fix.Tree([result, state.value])


def _children(node):
    for field in node._fields:
        value = getattr(node, field, None)
        if isinstance(value, ast.AST):
            yield value
        elif isinstance(value, list):
            for child in value:
                if isinstance(child, ast.AST):
                    yield child


def _walk(node):
    yield node
    for child in _children(node):
        yield from _walk(child)


def _targets(target):
    if isinstance(target, ast.Name):
        return {target.id}
    if isinstance(target, (ast.Tuple, ast.List)):
        return {name for child in target.elts for name in _targets(child)}
    raise TypeError("workflow assignment targets must be local names or unpacking targets")


def _writes(statement):
    if isinstance(statement, ast.Assign):
        return set().union(*map(_targets, statement.targets))
    if isinstance(statement, (ast.For, ast.AugAssign)):
        return _targets(statement.target)
    return set()


def _reads(node, bound=frozenset()):
    if isinstance(node, ast.Name):
        return {node.id} if isinstance(node.ctx, ast.Load) and node.id not in bound else set()
    if isinstance(node, (ast.ListComp, ast.SetComp, ast.DictComp, ast.GeneratorExp)):
        reads = set()
        for clause in node.generators:
            reads.update(_reads(clause.iter, bound))
            reads.update(_reads(clause.target, bound))
            bound = bound | {part.id for part in _walk(clause.target)
                             if isinstance(part, ast.Name) and isinstance(part.ctx, ast.Store)}
            for condition in clause.ifs:
                reads.update(_reads(condition, bound))
        results = (node.key, node.value) if isinstance(node, ast.DictComp) else (node.elt,)
        return reads.union(*(_reads(result, bound) for result in results))
    return set().union(*(_reads(child, bound) for child in _children(node)))


def _live_before(body, live, loop=None):
    for statement in reversed(body):
        if isinstance(statement, ast.If):
            live = (_reads(statement.test)
                    | _live_before(statement.body, live, loop)
                    | _live_before(statement.orelse, live, loop))
        elif isinstance(statement, (ast.For, ast.While)):
            after = live
            exhausted = _live_before(statement.orelse, after, loop)
            is_for = isinstance(statement, ast.For)
            if not is_for:
                exhausted = exhausted | _reads(statement.test)
            live = exhausted
            while True:
                body_live = _live_before(statement.body, live, (after, live))
                before = exhausted | (body_live - _writes(statement))
                if before == live:
                    break
                live = before
            if is_for:
                live = live | _reads(statement.iter)
        elif isinstance(statement, (ast.Break, ast.Continue)):
            live = loop[0 if isinstance(statement, ast.Break) else 1]
        elif isinstance(statement, (ast.Return, ast.Raise)):
            live = _reads(statement)
        elif isinstance(statement, ast.AugAssign):
            live = live | _writes(statement) | _reads(statement.value)
        else:
            live = (live - _writes(statement)) | _reads(statement)
    return live


def _statements(body):
    for statement in body:
        yield statement
        if isinstance(statement, (ast.If, ast.For, ast.While)):
            yield from _statements(statement.body)
            yield from _statements(statement.orelse)


def _split(body, parameters, local_names, context_name):
    if not body or not isinstance(body[-1], ast.Return) or body[-1].value is None:
        raise TypeError("a workflow must end with a Fix value return")
    live = set()
    tails = []
    for statement in reversed(body):
        value = getattr(statement, "value", None)
        boundary = isinstance(value, ast.Await) and isinstance(statement, (ast.Assign, ast.Expr))
        nodes = list(_walk(statement))
        if any(isinstance(node, (ast.NamedExpr, ast.Yield, ast.YieldFrom)) for node in nodes):
            raise TypeError("workflow bodies require explicit assignments and awaits")
        if any(isinstance(node, ast.Await) and (not boundary or node is not value)
               for node in nodes):
            raise TypeError("await must be a top-level statement or assignment")
        assignments = set()
        for nested in _statements([statement]):
            if not isinstance(nested, (ast.Assign, ast.AugAssign, ast.Expr, ast.Return, ast.Assert,
                                       ast.Pass, ast.If, ast.For, ast.While, ast.Break,
                                       ast.Continue, ast.Raise)):
                raise TypeError(f"unsupported workflow statement: {type(nested).__name__}")
            if isinstance(nested, ast.Return) and nested.value is None:
                raise TypeError("workflow return requires a Fix value")
            assignments.update(_writes(nested))
        if context_name in assignments:
            raise TypeError("cannot assign to the workflow environment")
        carry = (live - _writes(statement)) & local_names if boundary else set()
        tails.append((statement, boundary, carry, assignments))
        live = _live_before([statement], live)

    defined = set(parameters)
    segment = []
    for statement, boundary, carry, writes in reversed(tails):
        if boundary:
            if missing := carry - defined:
                names = ", ".join(sorted(missing))
                raise TypeError(f"workflow local may be read before assignment: {names}")
            yield segment, statement, sorted(carry)
            segment = []
        else:
            segment.append(statement)
        defined.update(writes)
    yield segment, None, []


def _parse(source):
    return compile(source, "<workflow>", "exec", ast.PyCF_ONLY_AST)


def _stage(name, parameters, state, reply, driver, after, body, boundary, carry, environment):
    args = [state, *([reply] if after else []), *parameters]
    stage = _parse(f"def {name}({', '.join(args)}, *, {driver}=None): pass").body[0]
    preamble = []
    if after:
        preamble.extend(_parse(f"{state}, {reply} = {driver}.resume({state}, {reply})").body)
        if environment:
            preamble.extend(_parse(f"if {state} is None: return {reply}").body)
    if environment:
        preamble.extend(_parse(f"{environment} = {driver}.context({state})").body)
    if isinstance(after, ast.Assign):
        resumed = _parse(f"{reply} = {reply}").body[0]
        resumed.targets = after.targets
        preamble.append(resumed)
    current = environment or state
    for statement in _statements(body):
        if isinstance(statement, ast.Return):
            finish = _parse(f"{driver}.finish({current}, {reply})").body[0].value
            finish.args[1] = statement.value
            statement.value = finish
    if boundary:
        forward = ", ".join([current, reply, *carry])
        finish = _parse(f"{driver}.advance({forward})").body[0].value
        finish.args[1] = boundary.value.value
        terminal = boundary
        result = ast.Return(value=finish)
        for field in ("lineno", "col_offset", "end_lineno", "end_col_offset"):
            setattr(result, field, getattr(terminal, field))
        body.append(result)
    stage.body = preamble + body
    return stage


def compile_flow(fn, adapter=None):
    if (fn.__globals__ is not fix._script_globals
            or fn.__qualname__ != fn.__name__ or fn.__closure__):
        raise TypeError("@do requires a top-level script function")
    module = _parse(bytes(fix._wrap(fix._source)).decode("utf-8"))
    definitions = [
        node for node in module.body
        if isinstance(node, ast.AsyncFunctionDef) and node.name == fn.__name__
    ]
    if len(definitions) != 1:
        raise TypeError("@do requires a unique top-level async function")
    function = definitions[0]
    signature = function.args
    if signature.defaults or signature.vararg or signature.kwarg or signature.kwonlyargs:
        raise TypeError("workflow parameters must be required positional parameters")
    parameters = [argument.arg for argument in signature.posonlyargs + signature.args]
    environment = None
    if adapter is not None:
        if not parameters:
            raise TypeError("a Flatware workflow requires an environment parameter")
        environment = parameters.pop(0)
    locals_ = set(fn.__code__.co_varnames) - {environment}
    segments = list(_split(function.body, parameters, locals_, environment))
    names = set(fn.__code__.co_names) | set(fn.__code__.co_varnames)
    internals = []
    for name in ("_fix_state", "_fix_reply", "_fix_driver"):
        while name in names:
            name += "_"
        internals.append(name)
        names.add(name)
    stages = []
    previous = None
    for index, (body, boundary, carry) in enumerate(segments):
        name = f"_fix_do_{function.lineno}_{index}"
        stages.append(_stage(name, parameters, *internals, previous, body, boundary, carry, environment))
        parameters = carry
        previous = boundary

    module.body = stages
    namespace = {}
    exec(compile(module, "<workflow>", "exec"), fn.__globals__, namespace)
    successor = None
    for stage in reversed(stages):
        current = namespace[stage.name]
        current.__kwdefaults__ = {internals[2]: _Step(adapter, successor)}
        successor = fix.function(current)

    def start(*args):
        state, args = (fix.Blob(), args) if adapter is None else adapter.initial(args)
        return fix.call(successor, state, *args)

    return start
