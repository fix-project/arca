#! *#($limit @label("python") $0)

import fix

name = "world"
if len(fix.argv) >= 2:
    name = fix.argv[1]

s = f"hello, {name}"

print(s)
fix.exit(s)
