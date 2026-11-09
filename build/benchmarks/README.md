# Guest microbenchmarks

The benchmarks use Criterion. Each operation is a named benchmark, with the
usual filtering, sampling, and numeric terminal output:

```sh
bazel run --config=release //:benchmarks
bazel run --config=release //fix:benchmarks
bazel run --config=release //fix/benchmarks:continuations
bazel run --config=release //fix/benchmarks:continuations -- 'canonicalize/0$'
bazel run --config=release //:benchmarks -- --list
```

Leaf suites own their case definitions, guest implementation, and fixtures:

| Target | Cases |
|---|---|
| `//arca/benchmarks:core` | Native call baseline, Arca creation, ELF loading, invocation |
| `//arca/benchmarks:continuations` | Arca effects, continuation cloning, snapshot writes |
| `//fix/benchmarks:invocation` | Fix invocation, certified invocation, invocation chains |
| `//fix/benchmarks:continuations` | Fix capture/resume, effects, canonicalization, replay |
| `//fix/benchmarks:scheduler` | Evaluation of eight independent applications |
| `//fix/benchmarks:sdk` | Rust SDK reads, Tree construction, invalidation, continuation-based evaluation |

`//arca:benchmarks` and `//fix:benchmarks` run their respective leaf suites;
`//:benchmarks` runs both. Each leaf is a separate Criterion executable with its
own guest image and starts one VM when a case matches. Aggregates forward
Criterion arguments and stop if a suite fails. They do not invoke Bazel again.
The former `//benchmarks:core` target aliases `//:benchmarks`.

Size-dependent operations are Criterion benchmark groups registered with
`bench_with_input`. Each group measures 0, 64 KiB, and 1 MiB inputs separately.

Defaults are 10 samples and 50 ms warm-up. Criterion groups use a 50 ms
measurement window for fast operations, 100 ms for Fix invocation, 200 ms for
Fix function-handled effects, 500 ms for Fix chains and replay, and 1 s for
checkpoint canonicalization. These budgets account for batch
setup and teardown during Criterion's wall-clock calibration; reported times
still exclude them.
Criterion controls iteration counts and sampling. Standard Criterion timing
flags override the defaults. The runner always discards baselines and disables
plots; it writes no benchmark result files.

Criterion runs on the host and uses `iter_custom` to measure operations inside
one KVM guest per leaf suite. The guest executes the requested iteration count
and returns its elapsed time through pipes. Fixtures are loaded from Bazel runfiles before the
guest accepts benchmark requests. VM startup, transport, and fixture setup are
outside the operation timer. Sampling and statistical analysis belong to Criterion.

Cases cover Arca creation/destruction, ELF loading, invocation, effects,
continuation cloning, and writes while retaining a snapshot. Effect fixtures
report their iteration through explicit requests. Fix cases cover invocation
with and without Apply coupons, a short invocation chain, kernel-handled and
replayable capture/resume, canonicalization, and replay.
Canonicalization starts from a fresh captured continuation. Function-handled effect
cases initialize the producer and handler once, then resume successive
continuations on both sides. Each continuation is used at most once; initialization is outside the timer. Continuation cases use
payloads of 0, 64 KiB, and 1 MiB and check/write one byte every 4 KiB between
suspensions.
The table case uses one externref per serialized Fix handle width (32 bytes)
of the size parameter; this describes logical payload, not native table storage.
It checks the last entry and writes the first entry between effects.

SDK cases execute up to 512 operations per invocation. They include amortized
invocation overhead. Tree construction includes loading its two children;
invalidation measures reclamation with the input root and empty scratch state.
SDK evaluation cases execute up to 32 operations per invocation through the Fix
evaluator, including capture, resumption, root reclamation, capture release, and
reading the result. They separately measure a Blob value, strict identification,
and strict application of the identity callable. The producer starts once before
the timer; each measured step evaluates its request and resumes to its next
request. Startup and the final resumption are outside the timer.

Snapshot cases retain a live copy while writing so that the original state
must remain usable. Assertions check fixture results rather than allocation
counts or checkpoint layouts.

The workload choices follow [Arca §6.1](https://katherinemohr.github.io/assets/publications/osdi26.pdf)
and [Fix §5.2](https://arxiv.org/html/2511.00205#S5.SS2). Defaults favor quick local
runs rather than reproducing the papers' experiment durations.
