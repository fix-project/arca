#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

void smp_init_cores(void);

void outb(uint16_t port, uint8_t byte);
uint8_t inb(uint16_t port);
void cpuid(uint32_t leaf, uint32_t *eax, uint32_t *ebx, uint32_t *ecx,
           uint32_t *edx);

void putc(char c);
void puts(char *s);
void putsn(char *s, size_t n);
void putx(unsigned x);
void *kmalloc(size_t bytes);
void halt(void);

void *acpi_get_local_apic(void);
unsigned acpi_nproc(void);
uint8_t acpi_processor_apic_id(unsigned processor);
uint8_t acpi_processor_id(unsigned processor);
