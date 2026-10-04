#ifndef MT19937_STREAM_H
#define MT19937_STREAM_H
#include <stddef.h>
#include <stdint.h>
typedef struct {
  uint64_t words[312];
  int index;
  uint64_t buffered;
  unsigned used;
} mt19937_stream;
void mt19937_seed(mt19937_stream *, uint64_t);
void mt19937_read(mt19937_stream *, uint8_t *, size_t);
#endif
