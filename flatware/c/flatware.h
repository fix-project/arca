#ifndef FLATWARE_H
#define FLATWARE_H
#include "third_party/mt19937/mt19937.h"
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

enum {
  BADF = 8,
  EXIST = 20,
  FAULT = 21,
  INVAL = 28,
  ISDIR = 31,
  NOENT = 44,
  NOTDIR = 54,
  NOTEMPTY = 55,
  NOTSUP = 58,
  OVERFLOW = 61,
  SPIPE = 70,
  NOTCAPABLE = 76
};
#define RIGHT(bit) (UINT64_C(1) << (bit))
#define READ RIGHT(1)
#define WRITE RIGHT(6)
#define SEEK RIGHT(2)
#define TELL RIGHT(5)
#define FILE_RIGHTS                                                            \
  (READ | WRITE | SEEK | TELL | RIGHT(0) | RIGHT(3) | RIGHT(4) | RIGHT(7) |    \
   RIGHT(8) | RIGHT(21) | RIGHT(22) | RIGHT(23) | RIGHT(27))
#define DIRECTORY_RIGHTS                                                       \
  (RIGHT(7) | RIGHT(0) | RIGHT(3) | RIGHT(4) |                                 \
   (((RIGHT(21) - 1) & ~(RIGHT(9) - 1))) | RIGHT(21) | RIGHT(23) | RIGHT(24) | \
   RIGHT(25) | RIGHT(26))
#define NONE SIZE_MAX

typedef struct {
  uint8_t *data;
  size_t len, cap;
} Bytes;
typedef struct {
  size_t *data;
  size_t len, cap;
} Indices;
typedef struct {
  uint64_t dev, ino;
  uint8_t filetype;
  uint64_t nlink, size, atim, mtim, ctim;
} Stat;
typedef struct {
  Bytes name, bytes;
  Indices children;
  Stat stat;
  size_t parent, input_parent, input_index;
  bool stream;
} Node;
typedef struct {
  size_t node;
  uint64_t offset, rights, inheriting;
  uint16_t flags;
  bool preopen, active;
} Descriptor;
typedef struct {
  Node *nodes;
  size_t count, cap;
  Indices roots;
  Descriptor *descriptors;
  size_t descriptor_count, descriptor_cap;
  uint64_t next_inode;
} Filesystem;
typedef struct {
  Bytes *arguments, *environment;
  size_t argument_count, environment_count;
  Filesystem fs;
  mt19937_stream random;
  uint64_t time;
  uint32_t exit_status;
} State;

void *malloc(size_t);
void *realloc(void *, size_t);
void free(void *);
void *memcpy(void *, const void *, size_t);
void *memmove(void *, const void *, size_t);
void *memset(void *, int, size_t);
void require(bool);
void *reserve(void *, size_t *, size_t, size_t);
void bytes_resize(Bytes *, size_t);
Bytes bytes_allocate(size_t);
Bytes bytes_copy(const uint8_t *, size_t);
void indices_push(Indices *, size_t);
bool bytes_equal(const Bytes *, const uint8_t *, size_t);
uint64_t load_le(const uint8_t *, size_t);
void put_le(uint8_t *, uint64_t, size_t);
uint64_t node_size(const Node *);
void node_modified(Node *, uint64_t);
uint32_t node_file(const Node *);
uint32_t node_directory(const Node *);
void fs_init(Filesystem *);
void fs_destroy(Filesystem *);
uint32_t fs_descriptor(Filesystem *, uint32_t, uint64_t, Descriptor **);
uint32_t fs_set_flags(Filesystem *, uint32_t, uint32_t);
uint32_t fs_resolve(Filesystem *, uint32_t, const Bytes *, size_t *);
uint32_t fs_open(Filesystem *, uint32_t, const Bytes *, uint32_t, uint64_t,
                 uint64_t, uint32_t, uint64_t, uint32_t *);
// I/O ranges borrow node storage until the next filesystem mutation.
// Write ranges must be filled before another filesystem operation.
uint32_t fs_read(Filesystem *, uint32_t, size_t, bool, uint64_t, uint64_t,
                 Bytes *);
uint32_t fs_write(Filesystem *, uint32_t, size_t, bool, uint64_t, uint64_t,
                  Bytes *);
uint32_t fs_seek(Filesystem *, uint32_t, int64_t, uint32_t, uint64_t *);
uint32_t fs_resize(Filesystem *, size_t, uint64_t, uint64_t);
uint32_t fs_mkdir(Filesystem *, uint32_t, const Bytes *, uint64_t);
uint32_t fs_unlink(Filesystem *, uint32_t, const Bytes *, bool, uint64_t);
uint32_t fs_rename(Filesystem *, uint32_t, const Bytes *, uint32_t,
                   const Bytes *, uint64_t);
void stat_encode(const Stat *, uint8_t[64]);
extern State state;
#endif
