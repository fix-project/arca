#! *#($limit @label("python") $0 $1)

import fix

fib = fix.callable()

@fix.function
def add(x: int, y: int) -> int:
    fix.advise("fix::title", f"{x} + {y}")
    return x + y

if __name__ == "__main__":
    n = int(fix.arity(1)[0])
    fix.advise("fix::title", f"fib({n})")
    fix.advise("fix::canonicalize", True)
    if n <= 1:
        fix.exit(n)
    x = fix.call(fib, n-1)
    y = fix.call(fib, n-2)
    fix.exit(fix.call(add, x, y))
