#!/usr/bin/env bash
set -euo pipefail

root="$TEST_SRCDIR/$TEST_WORKSPACE"
fix="$root/$1"
cd "$TEST_TMPDIR"
export FIX_UNSIGNED=true FIX_JOBS=2
export FIX_TRUST="$PWD/.fix/trust" FIX_CONFIG_DIR="$PWD/config" XDG_DATA_HOME="$PWD/data"

"$fix" init
"$fix" import "$root/$2"
"$fix" trust add-tagger collector
"$root/$3" add . "python=$root/$4" "guest=$root/$5" "advisor=$root/$6"
memory=$("$fix" create-blob --u64 1073741824)
script=$("$fix" create-blob "$root/$7")
scheduler=$("$fix" create-tree "$memory" python "$script")
"$fix" label set python-policy "$scheduler"

result=$("$fix" --scheduler python-policy --memoize never eval \
    '*#(268435456u64 @label("advisor") (268435456u64 @label("guest")) 42u64 2u64)')
test "$("$fix" read-blob --string "$result")" = "scheduled by Python"

cat > advice.py <<'PY'
import fix

(guest,) = fix.argv
fix.advise("fix::output-size", 42)
fix.advise("fix::parallel", fix.Blob(2))
fix.advise("fix::title", "café")
fix.advise("fix::canonicalize", True)
fix.exit(fix.call(guest, "python").strict())
PY
result=$("$fix" --scheduler python-policy --memoize never eval \
    '*#(2147483648u64 @label("python") @file("advice.py") @label("guest"))' 2>advice-progress)
test "$("$fix" read-blob --string "$result")" = "scheduled by Python"
grep -F 'START local: Apply café' advice-progress

if "$fix" --scheduler python-policy --memoize never eval \
    '*#(268435456u64 @label("guest") "fail")' >failure 2>&1; then
    exit 1
fi
grep -F 'ValueError: policy rejected the application' failure

cat > conversion.py <<'PY'
#! *#($limit @label("python") $0)
import fix

fix.exit(int(fix.Blob(b"wrong")))
PY
request=$("$fix" --memoize never run --limit=2G conversion.py 2>conversion-error)
mapfile -t fields < <("$fix" read-tree "$request")
test "${#fields[@]}" -eq 2
test "$("$fix" read-blob --string "${fields[0]}")" = Fix.error
"$fix" read-blob --string "${fields[1]}" > traceback
grep -F 'ValueError: integer conversion requires a 1-, 2-, 4-, or 8-byte Blob' traceback
grep -F 'Traceback (most recent call last):' conversion-error
grep -F 'ValueError: integer conversion requires a 1-, 2-, 4-, or 8-byte Blob' conversion-error
