# Profiling Fix

Capture a guest call graph with the normal command:

```sh
fix --perf run.perf.data -j 1 run /path/to/workload.fix
fix --perf precompile.perf.data -j 16 --memory 16384 init /path/to/seed.tar --precompile
```

Open the report:

```sh
perf report --no-inline -i run.perf.data
```

Optional display settings for self cost and callers:

```sh
perf report --no-inline -i run.perf.data --no-children --sort comm,dso,symbol
perf report --no-inline -i run.perf.data --children -g graph,0.5,caller
perf script --no-inline -i run.perf.data -F +period
```

`--perf` needs no external tool to record or export. It writes a Linux
`perf.data` file with `PERF_SAMPLE_CALLCHAIN`, per-vCPU CPU numbers and thread
names, monotonic timestamps, and nanosecond sample periods. Existing outputs
are not overwritten. The exact sampled ELF images, ambiguous image candidates, kernel, raw snapshots,
and timing metadata are retained in `PATH.d/`. Keep that directory beside the
profile so `perf` can resolve symbols. Absolute ELF paths are stored in the
maps; use `perf report --symfs` or recreate the paths when moving the capture.

The event `arca-vcpu-wall-ns` measures estimated vCPU **state wall time**, not
cycles or CPU execution time. It uses the software CPU-clock record layout
with an explicit event name. Weights include halted vCPUs and time spent in
KVM while the guest instruction pointer remains stopped. Sum over workers
is capacity time, so a 16-vCPU, 10-second interval can contain 160 seconds of
weight. Do not interpret those weights as an exclusive CPU-time partition.
Sample counts are not invocation counts. Use `--profile` alongside `--perf`
when request counts and coarse runtime spans are also needed.

Guest kernel and user ELFs use separate synthetic process address spaces to
preserve their overlapping virtual addresses; `perf` displays both as ordinary
ELF-backed mappings. The sample CPU identifies the original vCPU.

The sampler interrupts KVM at approximately 100 Hz per vCPU, with jitter. It
streams snapshots, keeping memory bounded. Raw snapshots require about
470 KiB/s per vCPU. The command's capture interval and export duration are
separate in `PATH.d/capture.json`. Output finalization and symbol copying
happen after guest execution. Sampling can affect scheduling and caches;
compare elapsed time with an unprofiled control.

Native export uses frame pointers. Assembly leaf routines and code compiled
without frame pointers can have incomplete chains. It retains 4 KiB stack
snapshots and register state for deeper offline unwinding:

```sh
python3 tools/profiling/export.py run.perf.data.d dsl.enhanced.perf.data \
  --kernel run.perf.data.d/kernel.elf \
  --images run.perf.data.d/images --unwind-cache ./unwind-cache
```

The optional exporter uses Python 3 and `llvm-objdump`. It caches conservative
x86-64 stack-depth metadata, recovers saved frame pointers, and stops at
unsupported control flow or snapshot boundaries. It does not guess missing
callers or ambiguous executable identities. Its summary reports coverage and
capture latency. The offline path corrects frame-pointer omissions in leaf
assembly without modifying the guest or rebuilding the toolchain. Add
`--active-only` to exclude sampled HLT and unavailable host/missing intervals;
this does not turn the remaining weights into CPU time. Raw `.bin.gz`
snapshots are accepted too.

A program's virtual address is insufficient to identify it. Both exporters
match sampled instruction bytes to exact ELF contents. Missing or ambiguous
images remain unresolved. Partial chains still support self cost and nearby
caller analysis, but inclusive costs of omitted ancestors are lower bounds.

`--no-inline` avoids very slow `addr2line` expansion on the large translated
bootstrap compiler. Native symbol names and recorded caller chains remain
available. Full host CPU profiling is separate, for example with a host-only
hardware event and `--call-graph dwarf` in Linux perf; software `cpu-clock:H`
can charge guest execution to KVM's entry loop and should not be used to
estimate host overhead.
