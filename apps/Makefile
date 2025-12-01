ARCA_MUSL_PATH ?= /opt/arca/musl

OUT_DIR ?= $(shell pwd)/.build
ARCA_MUSL_BUILD=$(OUT_DIR)/arca-musl-build
ARCA_MUSL=$(OUT_DIR)/arca-musl

.PHONY: all clean

CC = $(ARCA_MUSL)/bin/musl-gcc

SRC=$(wildcard *.c)
ELF=$(SRC:.c=.elf)

CFLAGS += -static

all: $(ELF)

clean:
	rm -rf *.elf $(ARCA_MUSL_BUILD) $(ARCA_MUSL)

$(ARCA_MUSL_BUILD)/Makefile:
	mkdir -p $(ARCA_MUSL_BUILD)
	mkdir -p $(ARCA_MUSL)
	cd $(ARCA_MUSL_BUILD) && $(shell pwd)/../../arca/modules/arca-musl/configure --prefix=$(ARCA_MUSL)

arca-musl: $(ARCA_MUSL_BUILD)/Makefile
	make -C $(ARCA_MUSL_BUILD)
	make -C $(ARCA_MUSL_BUILD) install

%.elf: %.c arca-musl
	$(CC) $(CFLAGS) $< -o $@
