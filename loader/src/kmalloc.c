#include "loader.h"

extern char _sheap, _eheap;
static char *heap_current = &_sheap;

void *kmalloc(size_t bytes) {
  void *old = heap_current;
  heap_current += bytes;
  if (heap_current >= &_eheap) {
    puts("ERROR: heap overflow\n");
    halt();
  }
  return old;
}
