"""Python bindings for the Fix Shell."""

import _fix
from _operator import index as _index


_INTEGER_WIDTHS = (1, 2, 4, 8)


def _encode_blob_data(data, width=None):
    if width is None and isinstance(data, bool):
        width = 1
    if isinstance(data, int):
        width = 8 if width is None else _index(width)
        if width not in _INTEGER_WIDTHS:
            raise ValueError("integer Blob width must be 1, 2, 4, or 8")
        return int.to_bytes(data, width, "little", signed=True)
    if width is not None:
        raise TypeError("width is only supported when constructing a Blob from an integer")
    if isinstance(data, str):
        return data.encode("utf-8")
    return data


class Value:
    __slots__ = ("_handle",)

    def __init__(self):
        raise TypeError("use a Fix constructor or value operation")

    @property
    def is_comparable(self):
        return self._handle.is_comparable()

    def equals(self, other):
        return self._handle.equals(_unwrap(other))

    def __eq__(self, other):
        if not isinstance(other, Value):
            return NotImplemented
        return self.is_comparable and other.is_comparable and self.equals(other)

    def eval(self):
        return _wrap(self._handle.eval())

    def advise(self, key, payload):
        payload = bytes(payload) if isinstance(payload, Blob) else _encode_blob_data(payload)
        self._handle.advise(key, payload)


class Object(Value):
    __slots__ = ()

    @property
    def data_type(self):
        return self._handle.data_type()

    def reference(self):
        return _wrap(self._handle.reference())


class Blob(Object):
    __slots__ = ()

    def __init__(self, data=b"", *, width=None):
        self._handle = _fix.blob(_encode_blob_data(data, width))

    def __len__(self):
        return self._handle.len()

    def __bytes__(self):
        return self.read()

    def __getitem__(self, index):
        if isinstance(index, slice):
            start, stop, step = index.indices(len(self))
            if step == 1:
                return self.read(start, max(0, stop - start))
            return bytes(self)[index]
        return self.read(_checked_index(self, index), 1)[0]

    def __str__(self):
        return bytes(self).decode("utf-8")

    def __int__(self):
        if len(self) not in _INTEGER_WIDTHS:
            raise ValueError("integer conversion requires a 1-, 2-, 4-, or 8-byte Blob")
        return int.from_bytes(bytes(self), "little", signed=True)

    def __bool__(self):
        data = self.read(size=max(_INTEGER_WIDTHS) + 1)
        if data in (b"true", b"True"):
            return True
        if data in (b"false", b"False"):
            return False
        if len(data) in _INTEGER_WIDTHS:
            return any(data)
        raise ValueError("boolean conversion requires an integer Blob or true/false text")

    def read(self, offset=0, size=None):
        offset = _index(offset)
        if offset < 0:
            raise ValueError("negative Blob offset")
        length = len(self)
        available = max(0, length - offset)
        if size is not None:
            size = _index(size)
            if size < 0:
                raise ValueError("negative Blob read size")
            available = min(available, size)
        return self._handle.read(min(offset, length), available)


class Tree(Object):
    __slots__ = ("_pairs",)

    def __init__(self, children=()):
        if isinstance(children, dict):
            children = (Tree(pair) for pair in children.items())
        self._handle = _fix.tree(_unwrap(_to_fix(child)) for child in children)
        self._pairs = None

    def __len__(self):
        return self._handle.len()

    def __getitem__(self, index):
        if isinstance(index, slice):
            return Tree(self[i] for i in range(*index.indices(len(self))))
        if isinstance(index, bytes):
            return self._dictionary()[index]
        return _wrap(self._handle.get(_checked_index(self, index)))

    def keys(self):
        return self._dictionary().keys()

    def _dictionary(self):
        if getattr(self, "_pairs", None) is None:
            self._pairs = _convert_argument(self, dict)
        return self._pairs

    @property
    def is_tag(self):
        return self._handle.is_tag()

    def application(self):
        return _wrap(self._handle.application())

    def selection(self):
        return _wrap(self._handle.selection())


class Reference(Value):
    __slots__ = ()

    def __len__(self):
        return self._handle.len()

    @property
    def data_type(self):
        return self._handle.data_type()

    def identification(self):
        return _wrap(self._handle.identification())

    def select(self, index, stop=None):
        """Build a selection Thunk for an index or a half-open range."""
        if stop is None:
            return _select(self, _checked_index(self, index))
        start = _checked_index(self, index, endpoint=True)
        stop = _checked_index(self, stop, endpoint=True)
        if start > stop:
            raise IndexError("Fix selection start exceeds stop")
        return _select(self, start, stop)


class Thunk(Value):
    __slots__ = ()

    def strict(self):
        return _wrap(self._handle.strict())

    def shallow(self):
        return _wrap(self._handle.shallow())

    def select(self, index, stop=None):
        """Build a selection Thunk targeting this Thunk's result."""
        bounds = (_index(index),) if stop is None else (_index(index), _index(stop))
        if any(bound < 0 for bound in bounds):
            raise IndexError("Thunk selection requires nonnegative bounds")
        return _select(self.shallow(), *bounds)

    def __getitem__(self, index):
        if isinstance(index, slice):
            if index.stop is None or (index.step is not None and _index(index.step) != 1):
                raise ValueError("Thunk slices require a stop and a step of one")
            return self.select(0 if index.start is None else index.start, index.stop)
        return self.select(index)

    __iter__ = None


class Encode(Value):
    __slots__ = ()

    @property
    def encode_type(self):
        return self._handle.encode_type()


_types = {
    "blob": Blob,
    "tree": Tree,
    "reference": Reference,
    "thunk": Thunk,
    "encode": Encode,
}


def _wrap(handle):
    value = object.__new__(_types[handle.kind()])
    value._handle = handle
    return value


def _unwrap(value):
    if not isinstance(value, Value) or not hasattr(value, "_handle"):
        raise TypeError("expected a Fix value")
    return value._handle


def _checked_index(value, index, *, endpoint=False):
    index = _index(index)
    size = len(value)
    if index < 0:
        index += size
    last = size if endpoint else size - 1
    if not 0 <= index <= last:
        raise IndexError(f"Fix {type(value).__name__} index out of range")
    return index


def _select(target, *bounds):
    return Tree([target, *(Blob(bound.to_bytes(8, "little")) for bound in bounds)]).selection()


def tag(fields):
    return _wrap(_fix.tag(map(_unwrap, fields)))


def combination():
    return _wrap(_fix.combination())


def effect(tree):
    """Submit an effect to Fix."""
    return _wrap(_fix.effect(_unwrap(tree)))


def advise(key, payload):
    """Emit advice about the current Fix application combination."""
    combination().advise(key, payload)


def call(function, *args, metadata=None):
    """Construct an application Thunk with lazy arguments."""
    if metadata is None:
        metadata = combination()[0]
    arguments = (_unwrap(_to_fix(argument)) for argument in args)
    return _wrap(_fix.call(_unwrap(metadata), _unwrap(function), arguments))


def do(fn):
    """Compile the await boundaries of a workflow into Fix calls."""
    from fix._do import compile_flow

    return compile_flow(fn)


def parallel(*operations):
    """Group Fix operations whose results are needed by the same step."""
    from fix._do import Parallel

    return Parallel(operations)


argv = ()
_functions = {}
_source = None
_script_globals = None


class _ScriptExit(BaseException):
    pass


def exit(value=b""):
    """Finish the script with a Fix value or a convertible Python value."""
    raise _ScriptExit(_unwrap(_to_fix(value)))


def arity(count):
    """Return argv, or exit with a partial application if too few arguments were supplied."""
    count = _index(count)
    if count < 0:
        raise ValueError("arity must be nonnegative")
    if len(argv) < count:
        exit(combination())
    return argv


def function(fn):
    """Replace a named Python function with its Fix callable Tree."""
    if not isinstance(fn, type(function)):
        raise TypeError("expected a named top-level script function")
    name = fn.__name__
    if (fn.__globals__ is not _script_globals or fn.__qualname__ != name
            or not name.isidentifier() or fn.__closure__):
        raise TypeError("expected a named top-level script function")
    if name in _functions:
        raise TypeError(f"duplicate Fix function {name!r}")
    _functions[name] = fn
    entry = Tree([_wrap(_source), Blob(name.encode("utf-8"))])
    return _script_callable(entry)


def callable():
    """Return the script's Fix callable with no arguments bound."""
    return _script_callable(_wrap(_source))


def _script_callable(entry):
    invocation = combination()
    interpreter = invocation[1]
    while not interpreter.is_tag:
        interpreter = interpreter[1]
    return Tree([invocation[0], interpreter, entry])


_BLOB_CONVERSIONS = (bytes, str, int, bool)
_CONTAINERS = (list, tuple, dict)
_STRICT_TYPES = _BLOB_CONVERSIONS + _CONTAINERS + (Blob, Tree, Object)
_ANNOTATED_TYPES = _STRICT_TYPES + (Reference, Thunk, Encode, Value)


def _annotation_shape(annotation):
    origin = getattr(annotation, "__origin__", None)
    if origin in _CONTAINERS:
        return origin, getattr(annotation, "__args__", None)
    for kind in _ANNOTATED_TYPES:
        if annotation is kind:
            return kind, None
        if isinstance(annotation, str) and annotation in (kind.__name__, f"{kind.__module__}.{kind.__name__}"):
            return kind, None
    return None, None


def _argument_annotations(code, annotations, count):
    names = code.co_varnames[:min(code.co_argcount, count)]
    if count > len(names):
        variadic = code.co_varnames[code.co_argcount + code.co_kwonlyargcount]
        names += (variadic,) * (count - len(names))
    return tuple(annotations.get(name) for name in names)


def _element_annotations(kind, parameters, size):
    if kind is dict:
        key, item = parameters if parameters is not None else (bytes, None)
        return (tuple[key, item],) * size
    if parameters is None:
        return (None,) * size
    if kind is list:
        item, = parameters
        return (item,) * size
    if len(parameters) == 2 and parameters[1] is Ellipsis:
        return (parameters[0],) * size
    if len(parameters) != size:
        raise ValueError(f"expected a Tree with {len(parameters)} tuple elements")
    return parameters


def _tree_elements(value):
    if not isinstance(value, Tree) or value.is_tag:
        raise TypeError("container conversion requires an untagged Fix Tree")
    return tuple(value)


def _prepare_argument(value, annotation):
    kind, parameters = _annotation_shape(annotation)
    if kind in _STRICT_TYPES or kind is Reference:
        prepared = _fix.prepare_argument(_unwrap(value), kind is Reference)
        if prepared is not None:
            return _wrap(prepared)
    if kind not in _CONTAINERS or (parameters is None and kind is not dict):
        return value
    children = _tree_elements(value)
    prepared = tuple(map(_prepare_argument, children, _element_annotations(kind, parameters, len(children))))
    if any(old is not new for old, new in zip(children, prepared)):
        return Tree(prepared)
    return value


def _convert_argument(value, annotation):
    kind, parameters = _annotation_shape(annotation)
    if kind is None:
        return value
    if kind in _CONTAINERS:
        children = _tree_elements(value)
        return kind(map(_convert_argument, children, _element_annotations(kind, parameters, len(children))))
    if kind in _BLOB_CONVERSIONS:
        if not isinstance(value, Blob):
            raise TypeError(f"expected a Blob for a {kind.__name__} argument")
        return kind(value)
    if not isinstance(value, kind):
        raise TypeError(f"expected a Fix {kind.__name__} argument")
    return value


def _to_fix(value):
    if isinstance(value, Value):
        return value
    if isinstance(value, _BLOB_CONVERSIONS):
        return Blob(value)
    if isinstance(value, _CONTAINERS):
        return Tree(value)
    raise TypeError("expected a Fix value, bytes, string, integer, boolean, list, tuple, or dict")


def _convert_return(value, annotation):
    kind, parameters = _annotation_shape(annotation)
    if kind is None:
        return _to_fix(value)
    if not isinstance(value, kind):
        raise TypeError(f"expected a {kind.__name__} return value")
    if kind in _BLOB_CONVERSIONS:
        return Blob(kind(value))
    if kind in _CONTAINERS:
        if parameters is None:
            return Tree(value)
        elements = value.items() if kind is dict else value
        return Tree(map(_convert_return, elements, _element_annotations(kind, parameters, len(value))))
    return value


def _run_script(code, namespace, raw_combination, raw_args, name):
    global argv, _script_globals
    _script_globals = namespace
    argv = tuple(map(_wrap, raw_args))
    if name is None:
        try:
            exec(code, namespace)
        except _ScriptExit as result:
            return result.args[0]
        return _unwrap(Blob())

    namespace["__name__"] = "_fix_script"
    exec(code, namespace)
    if name not in _functions:
        raise ValueError(f"unknown Fix function {name!r}")
    return _run_function(raw_combination, argv, _functions[name])


def _run_function(raw_combination, args, function):
    code = function.__code__
    arity = code.co_argcount
    required = arity - len(function.__defaults__ or ())
    if len(args) < required:
        return raw_combination
    count = len(args) if code.co_flags & 0x04 else min(arity, len(args))  # CO_VARARGS
    arguments = args[:count]
    return_annotation = None
    annotations = function.__annotations__
    if annotations:
        expected = _argument_annotations(code, annotations, count)
        prepared = tuple(map(_prepare_argument, arguments, expected))
        if any(old is not new for old, new in zip(arguments, prepared)):
            return _fix.reapply(raw_combination, map(_unwrap, prepared))
        arguments = tuple(map(_convert_argument, arguments, expected))
        return_annotation = annotations.get("return")
    result = _unwrap(_convert_return(function(*arguments), return_annotation))
    if len(args) > count:
        return _fix.apply_remaining(_unwrap(_wrap(raw_combination)[0]), result,
                                    map(_unwrap, args[count:]))
    return result
