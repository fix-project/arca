#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
runner="$root/$1"
probe="$root/$2"
ls="$root/$3"
wc="$root/$4"
cat="$root/$5"
head="$root/$6"
work="$TEST_TMPDIR/work"
mkdir -p "$work/input/source"
export TMPDIR="$TEST_TMPDIR"
printf original > "$work/input/input.txt"
ln -s missing "$work/input/dangling"
ln -s input.txt "$work/input/link"
ln -s ../input.txt "$work/input/source/link"
printf 'buffered\0input' > "$work/stdin"
status=0
"$runner" --dir "$work/input::/work" --env GREETING=hello --stdin "$work/stdin" "$probe" argument > "$work/stdout" 2> "$work/stderr" || status=$?
test "$status" = 23
cmp "$work/stdin" "$work/stdout"
grep -Fx 'guest diagnostic' "$work/stderr"
output=$(sed -n 's/^output directory: //p' "$work/stderr")
test -d "$output"
printf 'changed\0bytes' > "$work/expected"
cmp "$work/expected" "$output/work/nested/result"
cmp "$work/input/input.txt" "$output/work/renamed.txt"
test ! -e "$work/input/nested"
test ! -e "$work/input/renamed.txt"
test ! -e "$output/work/input.txt"
test "$(readlink "$output/work/nested/moved/dangling")" = missing
test "$(readlink "$output/work/nested/moved/link")" = ../input.txt
rm "$work/input/dangling" "$work/input/link"
rm -r "$work/input/source"

printf 'two words\nlast' > "$work/input/words"
printf '1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12' > "$work/input/lines"
ln -s words "$work/input/link"
"$ls" --dir "$work/input::/work" /work > "$work/stdout" 2> "$work/stderr"
printf 'input.txt\nlines\nlink\nwords\n' > "$work/expected"
cmp "$work/expected" "$work/stdout"
output=$(sed -n 's/^output directory: //p' "$work/stderr")
test "$(readlink "$output/work/link")" = words

"$wc" --dir "$work/input::/work" /work/words /work/input.txt > "$work/stdout" 2> "$work/stderr"
printf '1 3 14 /work/words\n0 1 8 /work/input.txt\n1 4 22 total\n' > "$work/expected"
cmp "$work/expected" "$work/stdout"

printf '%09000d\n' 0 > "$work/long"
"$wc" --stdin "$work/long" > "$work/stdout" 2> "$work/stderr"
printf '1 1 9001 -\n' > "$work/expected"
cmp "$work/expected" "$work/stdout"

"$cat" --stdin "$work/stdin" > "$work/stdout" 2> "$work/stderr"
cmp "$work/stdin" "$work/stdout"
"$cat" --dir "$work/input::/work" /work/words /work/input.txt > "$work/stdout" 2> "$work/stderr"
printf 'two words\nlastoriginal' > "$work/expected"
cmp "$work/expected" "$work/stdout"

"$head" --dir "$work/input::/work" /work/lines > "$work/stdout" 2> "$work/stderr"
printf '1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n' > "$work/expected"
cmp "$work/expected" "$work/stdout"
"$head" --stdin "$work/input/words" > "$work/stdout" 2> "$work/stderr"
cmp "$work/input/words" "$work/stdout"

status=0
"$cat" --dir "$work/input::/work" /work/missing > "$work/stdout" 2> "$work/stderr" || status=$?
test "$status" = 1
grep 'cat: /work/missing:' "$work/stderr"
output=$(sed -n 's/^output directory: //p' "$work/stderr")
cmp "$work/input/words" "$output/work/words"
