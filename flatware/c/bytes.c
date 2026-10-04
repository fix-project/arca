#include "flatware.h"
void require(bool condition) {
  if (!condition)
    __builtin_trap();
}
void *reserve(void *data, size_t *capacity, size_t wanted, size_t width) {
  if (wanted <= *capacity)
    return data;
  require(wanted <= SIZE_MAX / width);
  size_t grown = *capacity ? *capacity : 4;
  while (grown < wanted) {
    if (grown > SIZE_MAX / 2) {
      grown = wanted;
      break;
    }
    grown *= 2;
  }
  if (grown > SIZE_MAX / width)
    grown = wanted;
  void *next = realloc(data, grown * width);
  require(next != NULL);
  *capacity = grown;
  return next;
}
void bytes_resize(Bytes *bytes, size_t n) {
  bytes->data = reserve(bytes->data, &bytes->cap, n, 1);
  if (n > bytes->len)
    memset(bytes->data + bytes->len, 0, n - bytes->len);
  bytes->len = n;
}
Bytes bytes_allocate(size_t n) {
  uint8_t *data = malloc(n);
  require(data || !n);
  return (Bytes){.data = data, .len = n, .cap = n};
}
Bytes bytes_copy(const uint8_t *data, size_t n) {
  Bytes bytes = bytes_allocate(n);
  if (n)
    memcpy(bytes.data, data, n);
  return bytes;
}
void indices_push(Indices *indices, size_t value) {
  require(indices->len < SIZE_MAX);
  indices->data =
      reserve(indices->data, &indices->cap, indices->len + 1, sizeof(size_t));
  indices->data[indices->len++] = value;
}
bool bytes_equal(const Bytes *bytes, const uint8_t *data, size_t n) {
  if (bytes->len != n)
    return false;
  for (size_t i = 0; i < n; i++)
    if (bytes->data[i] != data[i])
      return false;
  return true;
}
uint64_t load_le(const uint8_t *p, size_t n) {
  uint64_t value = 0;
  for (size_t i = 0; i < n; i++)
    value |= (uint64_t)p[i] << (8 * i);
  return value;
}
void put_le(uint8_t *p, uint64_t value, size_t n) {
  for (size_t i = 0; i < n; i++)
    p[i] = (uint8_t)(value >> (8 * i));
}
