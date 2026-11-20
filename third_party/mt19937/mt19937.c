#include "mt19937.h"
void *memcpy(void *, const void *, size_t);
#include "mt19937-64.c"
_Static_assert(sizeof(mt) == sizeof(((mt19937_stream *)0)->words),
               "64-bit state words");
void mt19937_seed(mt19937_stream *state, uint64_t seed) {
  init_genrand64(seed);
  memcpy(state->words, mt, sizeof(mt));
  state->index = mti;
  state->buffered = 0;
  state->used = 8;
}
void mt19937_read(mt19937_stream *state, uint8_t *out, size_t len) {
  memcpy(mt, state->words, sizeof(mt));
  mti = state->index;
  for (size_t i = 0; i < len; i++) {
    if (state->used == 8) {
      state->buffered = genrand64_int64();
      state->used = 0;
    }
    out[i] = (uint8_t)(state->buffered >> (state->used++ * 8));
  }
  memcpy(state->words, mt, sizeof(mt));
  state->index = mti;
}
