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


class _Cursor:
    def __init__(self, sequence, position=0):
        self.sequence = sequence
        self.position = position

    def __next__(self):
        if self.position >= len(self.sequence):
            raise StopIteration
        item = self.sequence[self.position]
        self.position += 1
        return item


def _pack_frame(frame):
    seen, active = {}, set()

    def pack(value):
        kind = type(value)
        if isinstance(value, fix.Value) and fix._unwrap(value).is_value():
            return fix.Tree(["fix", value])
        if value is None:
            return fix.Tree(["none"])
        if kind in (bool, str, bytes):
            return fix.Tree([kind.__name__, value])
        if kind is int:
            return fix.Tree(["int", hex(value)])
        if kind is float:
            return fix.Tree(["float", value.hex()])
        if kind is complex:
            return fix.Tree(["complex", value.real.hex(), value.imag.hex()])
        if kind is range:
            return fix.Tree(["range", hex(value.start), hex(value.stop), hex(value.step)])
        identity = id(value)
        if identity in seen:
            return fix.Tree(["alias", seen[identity]])
        if identity in active:
            raise TypeError("cyclic Python state")
        active.add(identity)
        if kind is dict:
            fields = [fix.Tree([pack(key), pack(item)]) for key, item in value.items()]
            encoded = fix.Tree(["dict", fix.Tree(fields)])
        elif kind in (list, tuple, set, frozenset):
            encoded = fix.Tree([kind.__name__, fix.Tree(map(pack, value))])
        elif kind is _Cursor:
            encoded = fix.Tree(["cursor", pack(value.sequence), hex(value.position)])
        elif kind is bytearray:
            encoded = fix.Tree(["bytearray", bytes(value)])
        else:
            raise TypeError(f"{kind.__name__} is not Fix-serializable")
        active.remove(identity)
        seen[identity] = len(seen)
        return encoded

    fields = []
    for name, value in frame:
        try:
            fields.append(fix.Tree([name, pack(value)]))
        except TypeError as error:
            raise TypeError(f"workflow local {name!r}: {error}") from error
    return fix.Tree(fields)


def _unpack_frame(frame):
    seen = []

    def unpack(encoded):
        tag = str(encoded[0])
        if tag == "fix":
            return encoded[1]
        if tag == "none":
            return None
        if tag in ("bool", "str", "bytes"):
            return {"bool": bool, "str": str, "bytes": bytes}[tag](encoded[1])
        if tag == "int":
            return int(str(encoded[1]), 16)
        if tag == "float":
            return float.fromhex(str(encoded[1]))
        if tag == "complex":
            return complex(*(float.fromhex(str(part)) for part in encoded[1:]))
        if tag == "range":
            return range(*(int(str(part), 16) for part in encoded[1:]))
        if tag == "alias":
            return seen[int(encoded[1])]
        if tag == "dict":
            value = {unpack(pair[0]): unpack(pair[1]) for pair in encoded[1]}
        elif tag == "cursor":
            value = _Cursor(unpack(encoded[1]), int(str(encoded[2]), 16))
        elif tag == "bytearray":
            value = bytearray(bytes(encoded[1]))
        else:
            constructor = {"list": list, "tuple": tuple, "set": set, "frozenset": frozenset}[tag]
            value = constructor(map(unpack, encoded[1]))
        seen.append(value)
        return value

    return {str(pair[0]): unpack(pair[1]) for pair in frame}


class _Runtime:
    locals = staticmethod(locals)

    def __init__(self, adapter):
        self.adapter = adapter
        self.continuations = {}

    def context(self, state):
        return self.adapter._from_value(state)

    def resume(self, state, reply):
        if self.adapter is None:
            return state, reply
        return self.context(state)._resume(reply)

    def advance(self, label, state, operation, namespace):
        names, successor = self.continuations[label]
        frame = _pack_frame((name, namespace[name]) for name in names if name in namespace)
        if self.adapter is None:
            dependency = _dependency(operation)
        else:
            dependency = state._prepare(operation)
            state = state.value
        return fix.call(successor, state, dependency, frame)

    def finish(self, state, result):
        if self.adapter is None:
            return result
        return fix.Tree([result, state.value])

    def iterator(self, value):
        if type(value) in (fix.Tree, list, tuple, range, str, bytes, bytearray):
            return _Cursor(value)
        return iter(value)

    def next_item(self, iterator):
        try:
            return True, next(iterator)
        except StopIteration:
            return False, None

    def missing_return(self):
        raise TypeError("workflow must return a Fix value")


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
    if isinstance(target, ast.Starred):
        return _targets(target.value)
    return set()


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


def _parse(source):
    return compile(source, "<workflow>", "exec", ast.PyCF_ONLY_AST)


def _located(node, original):
    for field in ("lineno", "col_offset", "end_lineno", "end_col_offset"):
        setattr(node, field, getattr(original, field))
    return node


def _captures(pattern):
    names = set()
    for node in _walk(pattern):
        if isinstance(node, (ast.MatchAs, ast.MatchStar)) and node.name is not None:
            names.add(node.name)
        if isinstance(node, ast.MatchMapping) and node.rest is not None:
            names.add(node.rest)
    return names


class _Compiler:
    def __init__(self, names, environment):
        self.names = names
        self.environment = environment
        self.blocks = []
        self.resumes = set()
        self.temps = set()
        (self.state, self.reply, self.driver, self.block_id,
         self.frame) = (self.fresh(name) for name in ("state", "reply", "driver", "block", "frame"))

    def fresh(self, suffix):
        name = "_fix_" + suffix
        while name in self.names:
            name += "_"
        self.names.add(name)
        return name

    def temp(self, suffix):
        name = self.fresh(suffix)
        self.temps.add(name)
        return name

    def block(self, body, kind, *operands):
        label = len(self.blocks)
        self.blocks.append((body, kind, operands))
        return label

    def lower(self, body, after, loop=None):
        pending = []
        for statement in reversed(body):
            if isinstance(statement, (ast.Assign, ast.AugAssign, ast.Expr, ast.Assert, ast.Pass)):
                if not any(isinstance(node, ast.Await) for node in _walk(statement)):
                    pending.append(statement)
                    continue
            if pending:
                after = self.block(pending[::-1], "jump", after)
                pending.clear()
            after = self.lower_statement(statement, after, loop)
        return self.block(pending[::-1], "jump", after) if pending else after

    def await_before(self, expression, build):
        if not isinstance(expression, ast.Await):
            if any(isinstance(node, ast.Await) for node in _walk(expression)):
                raise TypeError("put nested awaits in separate workflow statements")
            return build(expression)
        if any(isinstance(node, ast.Await) for node in _walk(expression.value)):
            raise TypeError("put nested awaits in separate workflow statements")
        reply = _located(_parse(self.reply).body[0].value, expression)
        resume = self.block([], "jump", build(reply))
        self.resumes.add(resume)
        return self.block([], "await", resume, expression.value, expression)

    def lower_statement(self, statement, after, loop):
        if isinstance(statement, (ast.Assign, ast.AugAssign, ast.Expr)):
            def bind(value):
                statement.value = value
                return self.block([statement], "jump", after)
            return self.await_before(statement.value, bind)
        if isinstance(statement, ast.Return):
            if statement.value is None:
                raise TypeError("workflow return requires a Fix value")
            return self.await_before(statement.value,
                                     lambda value: self.block([], "return", value, statement))
        if isinstance(statement, ast.Raise):
            return self.block([statement], "end")
        if isinstance(statement, ast.If):
            yes = self.lower(statement.body, after, loop)
            no = self.lower(statement.orelse, after, loop)
            return self.await_before(statement.test,
                                     lambda test: self.block([], "branch", test, yes, no))
        if isinstance(statement, ast.Match):
            if any(isinstance(node, ast.Await) for case in statement.cases
                   if case.guard is not None for node in _walk(case.guard)):
                raise TypeError("workflow match guards must run within one stage")
            cases = [(case, self.lower(case.body, after, loop)) for case in statement.cases]
            return self.await_before(statement.subject,
                                     lambda subject: self.block([], "match", subject, cases, after))
        if isinstance(statement, (ast.For, ast.While)):
            head = self.block([], "end")
            done = self.lower(statement.orelse, after, loop)
            body = self.lower(statement.body, head, (after, head))
            if isinstance(statement, ast.While):
                test = self.await_before(statement.test,
                                         lambda value: self.block([], "branch", value, body, done))
                self.blocks[head] = ([], "jump", (test,))
                return head
            cursor, more, item = (self.temp(name) for name in ("cursor", "more", "item"))
            driver = self.driver
            advance = _parse(f"{more}, {item} = {driver}.next_item({cursor})").body
            bind = _parse(f"{item} = {item}").body[0]
            bind.targets = [statement.target]
            body = self.block([_located(bind, statement)], "jump", body)
            self.blocks[head] = (advance, "branch", (_parse(more).body[0].value, body, done))

            def iterator(value):
                start = _parse(f"{cursor} = {driver}.iterator(None)").body[0]
                start.value.args[0] = value
                return self.block([_located(start, statement)], "jump", head)
            return self.await_before(statement.iter, iterator)
        if isinstance(statement, ast.Break):
            return loop[0]
        if isinstance(statement, ast.Continue):
            return loop[1]
        raise TypeError(f"unsupported workflow statement: {type(statement).__name__}")

    def live(self, local_names):
        live = [set() for _ in self.blocks]
        while True:
            changed = False
            for label, (body, kind, operands) in enumerate(self.blocks):
                needed = self.tail_reads(live, kind, operands)
                for statement in reversed(body):
                    writes = _writes(statement)
                    reads = _reads(statement)
                    if isinstance(statement, ast.AugAssign):
                        reads |= writes
                    needed = (needed - writes) | reads
                needed = needed & local_names
                changed |= needed != live[label]
                live[label] = needed
            if not changed:
                return live

    def tail_reads(self, live, kind, operands):
        if kind == "jump":
            return live[operands[0]]
        if kind == "branch":
            test, yes, no = operands
            return _reads(test) | live[yes] | live[no]
        if kind == "match":
            subject, cases, after = operands
            needed = _reads(subject) | live[after]
            for case, target in cases:
                guard = _reads(case.guard) if case.guard is not None else set()
                needed |= _reads(case.pattern) | ((live[target] | guard) - _captures(case.pattern))
            return needed
        if kind == "await":
            resume, operation, _ = operands
            return _reads(operation) | live[resume]
        if kind == "return":
            return _reads(operands[0])
        return set()

    def jump(self, target):
        return _parse(f"{self.block_id} = {target}\ncontinue").body

    def emit_tail(self, kind, operands):
        current = self.environment or self.state
        if kind == "jump":
            return self.jump(operands[0])
        if kind == "branch":
            test, yes, no = operands
            branch = _parse("if False: pass").body[0]
            branch.test, branch.body, branch.orelse = test, self.jump(yes), self.jump(no)
            return [branch]
        if kind == "match":
            subject, cases, after = operands
            match = _parse("match None:\n    case _: pass").body[0]
            match.subject = subject
            match.cases = [ast.match_case(pattern=case.pattern, guard=case.guard, body=self.jump(target))
                           for case, target in cases]
            return [match, *self.jump(after)]
        if kind == "await":
            resume, operation, original = operands
            result = _parse(f"return {self.driver}.advance({resume}, {current}, None, "
                            f"{self.driver}.locals())").body[0]
            result.value.args[2] = operation
            return [_located(result, original)]
        if kind == "return":
            value, original = operands
            result = _parse(f"return {self.driver}.finish({current}, None)").body[0]
            result.value.args[1] = value
            return [_located(result, original)]
        return _parse(f"{self.driver}.missing_return()").body

    def emit(self, name, local_names):
        driver, state, reply, frame, label = (
            self.driver, self.state, self.reply, self.frame, self.block_id
        )
        function = _parse(f"def {name}({driver}, {label}, {state}, {reply}, {frame}): pass").body[0]
        preamble = []
        if self.environment:
            preamble.extend(_parse(f"{self.environment} = {driver}.context({state})").body)
        for local in sorted(local_names):
            key = repr(local)
            preamble.extend(_parse(f"if {key} in {frame}: {local} = {frame}[{key}]").body)
        dispatch = _parse("while True: pass").body[0]
        dispatch.body = []
        for index, (body, kind, operands) in enumerate(self.blocks):
            branch = _parse(f"if {label} == {index}: pass").body[0]
            branch.body = body + self.emit_tail(kind, operands)
            dispatch.body.append(branch)
        function.body = preamble + [dispatch]
        return function


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
    for node in _walk(function):
        if isinstance(node, (ast.NamedExpr, ast.Yield, ast.YieldFrom)):
            raise TypeError("workflow bodies require explicit assignments and awaits")
        if isinstance(node, ast.Name) and node.id == environment and isinstance(node.ctx, ast.Store):
            raise TypeError("cannot assign to the workflow environment")
        if isinstance(node, ast.Match) and any(environment in _captures(case.pattern)
                                                for case in node.cases):
            raise TypeError("cannot assign to the workflow environment")
    names = set(fn.__code__.co_names) | set(fn.__code__.co_varnames)
    compiler = _Compiler(names, environment)
    entry = compiler.lower(function.body, compiler.block([], "end"))
    local_names = (set(fn.__code__.co_varnames) | compiler.temps) - {environment}
    live = compiler.live(local_names)
    prefix = f"_fix_do_{function.lineno}"
    dispatch = compiler.emit(prefix, local_names)
    wrappers = {}
    for label in sorted(compiler.resumes):
        name = f"{prefix}_{label}"
        wrappers[label] = _parse(
            f"def {name}(state, reply, frame, *, runtime=None, dispatch=None):\n"
            "    state, reply = runtime.resume(state, reply)\n"
            "    if state is None: return reply\n"
            f"    return dispatch(runtime, {label}, state, reply, frame)"
        ).body[0]
    state, driver = compiler.state, compiler.driver
    invoke_name = compiler.fresh("invoke")
    fields = ", ".join(f"{name!r}: {name}" for name in parameters)
    entry_wrapper = _parse(
        f"def {prefix}_entry({', '.join([state, *parameters])}, *, {driver}=None, {invoke_name}=None):\n"
        f"    return {invoke_name}({driver}, {entry}, {state}, None, {{{fields}}})"
    ).body[0]
    module.body = [dispatch, entry_wrapper, *wrappers.values()]
    namespace = {}
    exec(compile(module, "<workflow>", "exec"), fn.__globals__, namespace)
    runtime = _Runtime(adapter)
    dispatch = namespace[prefix]

    def invoke(runtime, label, state, reply, frame):
        return dispatch(runtime, label, state, reply, _unpack_frame(frame))

    for label, wrapper in wrappers.items():
        function = namespace[wrapper.name]
        function.__kwdefaults__ = dict(runtime=runtime, dispatch=invoke)
        runtime.continuations[label] = sorted(live[label]), fix.function(function)
    entry_function = namespace[entry_wrapper.name]
    entry_function.__kwdefaults__ = {driver: runtime, invoke_name: dispatch}
    start_function = fix.function(entry_function)

    def start(*args):
        state, args = (fix.Blob(), args) if adapter is None else adapter.initial(args)
        return fix.call(start_function, state, *args)

    return start
