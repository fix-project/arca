.PHONY: all clean

TARGET=i686-unknown-none
CC=clang -target $(TARGET) -MMD
LD=clang -target $(TARGET)
CFLAGS=-ffreestanding -O3 -march=i586
LDFLAGS=-nostartfiles -nostdlib
OBJDUMP=llvm-objdump

ASM=$(wildcard *.S)
SRC=$(wildcard *.c)
OBJ=$(patsubst %.S,%.o,$(ASM)) $(patsubst %.c,%.o,$(SRC))

all: loader.elf loader.list

loader.elf: memmap.ld $(OBJ)
	$(LD) $(LDFLAGS) -T $^ -o $@

loader.list: loader.elf
	$(OBJDUMP) -D $< -M intel > $@

-include $(wildcard *.d)

clean:
	rm -rf *.o *.elf *.d

.PHONY: run
run: loader.elf loader.list
	qemu-kvm \
		-machine microvm \
		-kernel $< \
		-monitor none \
		-serial stdio \
		-nographic \
		-no-reboot \
		-smp $(shell nproc) \
		-m 4G \
		-bios /usr/share/qemu/qboot.rom
