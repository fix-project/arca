#include "loader.h"

struct sdt {
  uint32_t signature;
  uint32_t length;
  uint8_t revision;
  uint8_t checksum;
  char oem_id[6];
  char oem_table_id[8];
  uint32_t oem_revision;
  uint32_t creator_id;
  uint32_t creator_revision;
} __attribute__((packed));

struct xsdt {
  struct sdt header;
  uint64_t entries[];
} __attribute__((packed));

struct xsdp {
  uint64_t signature;
  uint8_t checksum;
  char oem_id[6];
  uint8_t revision;
  struct rdst *rsdt_address;

  uint32_t length;
  uint64_t xsdt_address;
  uint8_t extended_checksum;
  uint32_t : 24;
} __attribute__((packed));

struct processor_local_apic {
  uint8_t acpi_processor_id;
  uint8_t apic_id;
  uint32_t flags;
} __attribute__((packed));

struct madt_header {
  uint8_t type;
  uint8_t length;
  union {
    struct processor_local_apic processor_local_apic; // type == 0
  };
} __attribute__((packed));

struct madt {
  struct sdt header;
  void *local_apic_address;
  uint32_t flags;
  struct madt_header first_entry;
} __attribute__((packed));

_Static_assert(sizeof(struct xsdt *) == sizeof(uint32_t), "not 32 bits");

static struct xsdp *xsdp;
static struct xsdt *xsdt;

static struct xsdp *search(unsigned long start, unsigned long end) {
  const char *expected_signature_string = "RSD PTR ";
  const uint64_t expected_signature = *(uint64_t *)expected_signature_string;
  volatile struct xsdp *candidate = (void *)start;
  while ((void *)candidate < (void *)end) {
    uint64_t signature = candidate->signature;
    if (signature != 0) {
      if (signature == expected_signature) {
        // TODO: verify checksum
        return (struct xsdp *)candidate;
      }
    }
    candidate = (void *)candidate + 16;
  }
  return NULL;
}

struct xsdp *acpi_xsdp_get(void) {
  if (xsdp) {
    return xsdp;
  }
  xsdp = search(0x00080000, 0x000A0000);
  if (!xsdp) {
    xsdp = search(0x000E0000, 0x00100000);
  }
  if (!xsdp) {
    puts("ERROR (loader): could not find root system description pointer\n");
    halt();
  }
  return xsdp;
}

struct xsdt *acpi_xsdt_get(void) {
  if (xsdt) {
    return xsdt;
  }
  struct xsdp *xsdp = acpi_xsdp_get();
  xsdt = (void *)(uint32_t)xsdp->xsdt_address;
  return xsdt;
}

struct sdt *acpi_sdt_get(char *name) {
  struct xsdt *xsdt = acpi_xsdt_get();
  if (xsdt->header.signature != 0x54445358) {
    puts("ERROR (loader): invalid XSDT signature\n");
    halt();
  }
  size_t length =
      (xsdt->header.length - sizeof(xsdt->header)) / sizeof(xsdt->entries[0]);
  for (size_t i = 0; i < length; i++) {
    struct sdt *p = (void *)(uint32_t)xsdt->entries[i];
    uint32_t x;
    __builtin_memcpy(&x, name, 4);
    if (p->signature == x) {
      return p;
    }
  }
  return NULL;
}

static void *local_apic = NULL;

static unsigned nproc = 0;
static struct processor_local_apic *processor_info = NULL;

static void apic_init(void) {
  struct madt *madt = (struct madt *)acpi_sdt_get("APIC");
  if (!madt) {
    puts("ERROR (loader): unable to find Multiple APIC Description Table\n");
    halt();
  }
  local_apic = madt->local_apic_address;

  struct madt_header *entry = &madt->first_entry;
  while ((void *)entry < (void *)madt + madt->header.length) {
    if (entry->type == 0) {
      nproc++;
    }
    entry = (void *)(entry) + entry->length;
  }

  processor_info = kmalloc(sizeof(struct processor_local_apic) * nproc);

  entry = &madt->first_entry;
  int i = 0;
  while ((void *)entry < (void *)madt + madt->header.length) {
    if (entry->type == 0) {
      processor_info[i] = entry->processor_local_apic;
      i++;
    }
    entry = (void *)(entry) + entry->length;
  }
}

void *acpi_get_local_apic(void) {
  if (!local_apic) {
    apic_init();
  }
  return local_apic;
}

unsigned acpi_nproc(void) {
  if (processor_info == NULL) {
    apic_init();
  }
  return nproc;
}

uint8_t acpi_processor_apic_id(unsigned processor) {
  if (processor_info == NULL) {
    apic_init();
  }
  return processor_info[processor].apic_id;
}

uint8_t acpi_processor_id(unsigned processor) {
  if (processor_info == NULL) {
    apic_init();
  }
  return processor_info[processor].acpi_processor_id;
}
