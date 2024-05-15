#pragma once
#include <stdint.h>

struct pml4e {
  bool P_present : 1;
  bool RW_read_write : 1;
  bool US_user_supervisor : 1;
  bool PWT_write_through : 1;
  bool PCD_cache_disable : 1;
  bool A_accessed : 1;
  bool : 1;     // available
  unsigned : 2; // zero
  unsigned : 3; // available
  unsigned long long addr : 40;
  unsigned : 11; // available
  bool NX_execute_disable : 1;
} __attribute__((packed));
_Static_assert(sizeof(struct pml4e) == 8, "Page Map Level 4 Entry not 64 bits");

struct pdpe {
  bool P_present : 1;
  bool RW_read_write : 1;
  bool US_user_supervisor : 1;
  bool PWT_write_through : 1;
  bool PCD_cache_disable : 1;
  bool A_accessed : 1;
  bool D_dirty : 1;
  bool PS_page_size : 1;
  bool G_global : 1;
  unsigned : 3; // available
  bool PAT_page_attribute_table : 1;
  unsigned : 17; // zero
  unsigned long long addr : 22;
  unsigned : 7; // available
  unsigned MPK_memory_protection_key : 4;
  bool NX_execute_disable : 1;
} __attribute__((packed));
_Static_assert(sizeof(struct pdpe) == 8,
               "Page Directory Pointer Entry not 64 bits");

struct gdtr {
  uint16_t limit;
  uint32_t offset;
} __attribute__((packed));

struct access_byte {
  bool A_accessed : 1;
  bool RW_read_write : 1;
  bool DC_direction_conforming : 1;
  bool E_executable : 1;
  bool S_not_system : 1;
  uint8_t DPL_privilege_level : 2;
  bool P_present : 1;
} __attribute__((packed));
_Static_assert(sizeof(struct access_byte) == 1, "Access Byte not 8 bits");

struct system_access_byte {
  uint8_t type : 4;
  bool S_not_system : 1;
  uint8_t DPL_privilege_level : 2;
  bool P_present : 1;
} __attribute__((packed));
_Static_assert(sizeof(struct access_byte) == 1, "Access Byte not 8 bits");

struct segment_descriptor {
  uint16_t limit_0_15;
  uint32_t base_0_23 : 24;
  union {
    struct access_byte access;
    struct system_access_byte system_access;
  };
  uint8_t limit_16_19 : 4;
  bool : 1;
  bool L_long_mode : 1;
  bool DB_size_32 : 1;
  bool G_granularity : 1;
  uint8_t base_24_31 : 8;
} __attribute__((packed));
_Static_assert(sizeof(struct segment_descriptor) == 8,
               "Segment Descriptor not 64 bits");

union gdte {
  struct segment_descriptor descriptor;
  struct {
    uint32_t base_32_63;
    uint32_t : 32;
  } address;
} __attribute__((packed));
_Static_assert(sizeof(union gdte) == 8, "GDT Entry not 64 bits");
