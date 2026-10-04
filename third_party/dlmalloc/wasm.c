#include <stddef.h>
#include <stdint.h>
#define LACKS_SYS_TYPES_H
#define EINVAL 22
#define ENOMEM 12
#define LACKS_ERRNO_H
#define LACKS_TIME_H
#define LACKS_STDLIB_H
#define LACKS_STRING_H
#define LACKS_STRINGS_H
#define LACKS_UNISTD_H
#define USE_LOCKS 0
#define HAVE_MMAP 0
#define HAVE_MREMAP 0
#define NO_MALLOC_STATS 1
#define MALLOC_FAILURE_ACTION
#define ABORT __builtin_trap()
#define malloc_getpagesize 65536
#define MORECORE_CANNOT_TRIM 1
#define MORECORE wasm_morecore
static void *wasm_morecore(ptrdiff_t size) {
  if (size < 0)
    return (void *)-1;
  size_t pages = (size_t)size / 65536;
  if ((size_t)size % 65536)
    return (void *)-1;
  size_t previous = pages ? __builtin_wasm_memory_grow(0, pages)
                          : __builtin_wasm_memory_size(0);
  return previous > UINTPTR_MAX / 65536 ? (void *)-1
                                        : (void *)(previous * 65536);
}
void *memcpy(void *destination, const void *source, size_t size) {
  return __builtin_memcpy(destination, source, size);
}
void *memmove(void *destination, const void *source, size_t size) {
  return __builtin_memmove(destination, source, size);
}
void *memset(void *destination, int byte, size_t size) {
  return __builtin_memset(destination, byte, size);
}
#include "malloc.c"
