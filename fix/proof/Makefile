.DEFAULT_GOAL := all
.PHONY: all

wasm_modules := Backend Entry Execution Guards Layout
sources := $(filter-out fix/proof/collector/wasm/%,$(shell find fix/proof -maxdepth 2 -name '*.v')) \
	$(addprefix fix/proof/collector/wasm/,$(addsuffix .v,$(wasm_modules)))
binary_source := bazel-bin/fix/proof/collector/wasm/Binary.v
binary_object := fix/proof/collector/wasm/Binary.vo

all:
	bazel build //fix/proof/collector/wasm:binary_source
	$(MAKE) -f fix/proof/Makefile $(binary_object)
	rocq makefile -f _CoqProject $(sources) -o Makefile.rocq
	$(MAKE) -f Makefile.rocq all

$(binary_object): $(binary_source)
	rocq compile -q -noglob -Q $(dir $(binary_source)) FixProof.Collector.wasm \
		-Q $(dir $@) FixProof.Collector.wasm -o $@ $<
