#include "flatware.h"
#include <assert.h>

static uint8_t guest_memory[65536];
void guest_read(uint32_t p, void *out, uint32_t len) {
  assert(p <= sizeof(guest_memory) && len <= sizeof(guest_memory) - p);
  memcpy(out, guest_memory + p, len);
}
void guest_write(uint32_t p, const void *in, uint32_t len) {
  assert(p <= sizeof(guest_memory) && len <= sizeof(guest_memory) - p);
  memcpy(guest_memory + p, in, len);
}
uint32_t guest_size(void) { return 1; }
__attribute__((noreturn)) void guest_exit(void) { __builtin_trap(); }

uint32_t fd_pread(uint32_t, uint32_t, uint32_t, uint64_t, uint32_t);
uint32_t fd_pwrite(uint32_t, uint32_t, uint32_t, uint64_t, uint32_t);
uint32_t fd_read(uint32_t, uint32_t, uint32_t, uint32_t);
uint32_t fd_close(uint32_t);
uint32_t fd_renumber(uint32_t, uint32_t);
uint32_t path_filestat_get(uint32_t, uint32_t, uint32_t, uint32_t, uint32_t);
uint32_t path_filestat_set_times(uint32_t, uint32_t, uint32_t, uint32_t,
                                 uint64_t, uint64_t, uint32_t);
static Filesystem initial_fs(void) {
  Filesystem fs = {0};
  fs.nodes = reserve(fs.nodes, &fs.cap, 4, sizeof(Node));
  fs.count = 4;
  memset(fs.nodes, 0, 4 * sizeof(Node));
  const char *names[] = {"stdin", "stdout", "stderr", "/work"};
  size_t lengths[] = {5, 6, 6, 5};
  for (size_t i = 0; i < 4; i++) {
    fs.nodes[i] =
        (Node){.name = bytes_copy((const uint8_t *)names[i], lengths[i]),
               .stat = {.ino = i,
                        .nlink = 1,
                        .atim = 10,
                        .mtim = 10,
                        .ctim = 10,
                        .filetype = i < 3 ? 2 : 3},
               .parent = NONE,
               .input_parent = NONE,
               .stream = i < 3};
    if (i < 3)
      fs.nodes[i].bytes =
          bytes_copy((const uint8_t *)(i ? "prefix:" : "input"), i ? 7 : 5);
    indices_push(&fs.roots, i);
  }
  fs_init(&fs);
  return fs;
}
static Bytes text(const char *s, size_t n) {
  return (Bytes){.data = (uint8_t *)s, .len = n};
}
static uint32_t write_bytes(Filesystem *fs, uint32_t fd, const Bytes *input,
                            bool positioned, uint64_t offset, uint64_t time) {
  Bytes output;
  uint32_t error =
      fs_write(fs, fd, input->len, positioned, offset, time, &output);
  if (!error && input->len)
    memcpy(output.data, input->data, input->len);
  return error;
}
static void files_and_preopens(void) {
  Filesystem fs = initial_fs();
  Bytes path = text("nested", 6);
  assert(fs_mkdir(&fs, 3, &path, 11) == 0);
  path = text("nested/file", 11);
  uint32_t fd;
  assert(fs_open(&fs, 3, &path, 1, FILE_RIGHTS, 0, 0, 12, &fd) == 0);
  Bytes data = text("first", 5);
  assert(write_bytes(&fs, fd, &data, false, 0, 13) == 0);
  uint64_t offset;
  assert(fs_seek(&fs, fd, 2, 2, &offset) == 0 && offset == 7);
  data = text("last", 4);
  assert(write_bytes(&fs, fd, &data, false, 0, 14) == 0);
  assert(fs_seek(&fs, fd, 0, 0, &offset) == 0);
  Bytes output = {0};
  assert(fs_read(&fs, fd, 100, false, 0, 15, &output) == 0);
  assert(bytes_equal(&output, (const uint8_t *)"first\0\0last", 11));
  output = (Bytes){0};
  assert(fs_read(&fs, fd, 2, true, 7, 16, &output) == 0 &&
         bytes_equal(&output, (const uint8_t *)"la", 2));
  Bytes renamed = text("renamed", 7);
  assert(fs_rename(&fs, 3, &path, 3, &renamed, 18) == 0);
  size_t node;
  assert(fs_resolve(&fs, 3, &renamed, &node) == 0 &&
         fs.nodes[node].stat.size == 11 && fs.nodes[node].stat.ctim == 18);
  path = text("nested", 6);
  assert(fs_unlink(&fs, 3, &path, true, 19) == 0);
  assert(fs_unlink(&fs, 3, &renamed, false, 20) == 0 &&
         fs.nodes[3].children.len == 0);
  output = (Bytes){0};
  assert(fs_read(&fs, fd, 4, true, 0, 21, &output) == 0 &&
         bytes_equal(&output, (const uint8_t *)"firs", 4));
  assert(fs.nodes[node].stat.nlink == 0);
  fs_destroy(&fs);
}
static void capabilities(void) {
  Filesystem fs = initial_fs();
  Bytes path = text("nested", 6);
  assert(fs_mkdir(&fs, 3, &path, 11) == 0);
  uint32_t directory;
  assert(fs_open(&fs, 3, &path, 2, DIRECTORY_RIGHTS,
                 FILE_RIGHTS | DIRECTORY_RIGHTS, 0, 12, &directory) == 0);
  Bytes outside = text("../outside", 10);
  size_t node;
  assert(fs_resolve(&fs, directory, &outside, &node) == NOTCAPABLE);
  outside = text("/outside", 8);
  assert(fs_resolve(&fs, 3, &outside, &node) == NOTCAPABLE);
  Bytes child = text("child", 5);
  assert(fs_rename(&fs, 3, &path, directory, &child, 13) == INVAL);
  path = text("file", 4);
  uint32_t fd, reader;
  assert(fs_open(&fs, 3, &path, 1, READ | WRITE, 0, 0, 14, &fd) == 0);
  Bytes data = text("data", 4);
  assert(write_bytes(&fs, fd, &data, false, 0, 15) == 0);
  assert(fs_open(&fs, 3, &path, 0, READ, 0, 0, 16, &reader) == 0);
  data = text("!", 1);
  assert(write_bytes(&fs, reader, &data, false, 0, 17) == NOTCAPABLE);
  uint32_t broad;
  assert(fs_open(&fs, 3, &path, 0, FILE_RIGHTS | DIRECTORY_RIGHTS,
                 FILE_RIGHTS | DIRECTORY_RIGHTS, 0, 18, &broad) == 0);
  assert(fs.descriptors[broad].rights == FILE_RIGHTS &&
         fs.descriptors[broad].inheriting == 0);
  assert(fs_open(&fs, 3, &path, 5, READ, 0, 0, 19, &broad) == EXIST);
  fs_destroy(&fs);
}
static void descriptor_flags(void) {
  Filesystem fs = initial_fs();
  Bytes path = text(".", 1);
  uint32_t directory, fd;
  assert(fs_open(&fs, 3, &path, 2, DIRECTORY_RIGHTS, FILE_RIGHTS, 4, 11,
                 &directory) == 0 &&
         fs.descriptors[directory].flags == 4);
  path = text("file", 4);
  assert(fs_open(&fs, directory, &path, 1, FILE_RIGHTS, 0, 30, 12, &fd) == 0 &&
         fs.descriptors[fd].flags == 30);
  Bytes data = text("first", 5);
  assert(write_bytes(&fs, fd, &data, false, 0, 13) == 0);
  uint64_t position;
  assert(fs_seek(&fs, fd, 0, 0, &position) == 0);
  assert(fs_set_flags(&fs, fd, 31) == 0);
  data = text(" last", 5);
  assert(write_bytes(&fs, fd, &data, false, 0, 14) == 0);
  Bytes output = {0};
  assert(fs_read(&fs, fd, 100, true, 0, 15, &output) == 0 &&
         bytes_equal(&output, (const uint8_t *)"first last", 10));
  assert(fs_set_flags(&fs, fd, 32) == INVAL && fs.descriptors[fd].flags == 31);
  uint32_t reader;
  assert(fs_open(&fs, directory, &path, 0, READ, 0, 32, 16, &reader) == INVAL);
  assert(fs_open(&fs, directory, &path, 0, READ, 0, 4, 17, &reader) == 0);
  assert(fs_set_flags(&fs, reader, 0) == NOTCAPABLE);
  fs_destroy(&fs);
}
static void symlink_metadata(void) {
  state = (State){.fs = initial_fs(), .time = 10};
  Bytes path = text("link", 4);
  uint32_t fd;
  assert(fs_open(&state.fs, 3, &path, 1, FILE_RIGHTS, 0, 0, 11, &fd) == 0);
  Node *link = &state.fs.nodes[state.fs.descriptors[fd].node];
  link->stat.filetype = 7;
  link->stat.size = 7;
  link->bytes = bytes_copy((const uint8_t *)"missing", 7);
  memcpy(guest_memory, "link", 4);
  memset(guest_memory + 32, 0xab, 64);
  assert(path_filestat_get(3, 1, 0, 4, 32) == NOTSUP);
  assert(guest_memory[32] == 0xab);
  assert(path_filestat_set_times(3, 1, 0, 4, 100, 200, 5) == NOTSUP);
  assert(link->stat.atim == 11 && link->stat.mtim == 11);
  assert(path_filestat_get(3, 0, 0, 4, 32) == 0 && guest_memory[48] == 7);
  assert(path_filestat_set_times(3, 0, 0, 4, 100, 200, 5) == 0);
  assert(link->stat.atim == 100 && link->stat.mtim == 200);
  memcpy(guest_memory, ".", 1);
  assert(path_filestat_get(3, 1, 0, 1, 32) == 0 && guest_memory[48] == 3);
  assert(path_filestat_set_times(3, 1, 0, 1, 300, 400, 5) == 0);
  assert(state.fs.nodes[3].stat.atim == 300 &&
         state.fs.nodes[3].stat.mtim == 400);
  fs_destroy(&state.fs);
}
static void random_stream(void) {
  const uint8_t expected[] = {
      0x3e, 0xdc, 0x41, 0xcb, 0xc5, 0x37, 0xe8, 0x28, 0x8b, 0xf9, 0x40,
      0x3e, 0x7c, 0x3a, 0xfd, 0xfd, 0xb9, 0xe8, 0x32, 0xf0, 0x17, 0x32,
      0x21, 0xa,  0xee, 0xfc, 0xe3, 0xce, 0x3,  0x69, 0xf5, 0x98, 0xac,
      0x25, 0x7,  0x3b, 0x13, 0x30, 0xd3, 0x8a, 0xee, 0xe9, 0x5f, 0xfd,
      0x2a, 0x6,  0xa2, 0xe,  0x2f, 0xe1, 0x2a, 0x4,  0xf3, 0xd7, 0xab,
      0xa1, 0xe8, 0x46, 0x82, 0x45, 0x45, 0x1e, 0x6f, 0x6c};
  uint8_t whole[5001], chunked[5001];
  mt19937_stream a, b;
  mt19937_seed(&a, 0);
  mt19937_seed(&b, 0);
  mt19937_read(&a, whole, sizeof(whole));
  for (size_t i = 0; i < sizeof(expected); i++)
    assert(whole[i] == expected[i]);
  for (size_t offset = 0; offset < sizeof(chunked); offset += 13) {
    size_t len = sizeof(chunked) - offset;
    if (len > 13)
      len = 13;
    mt19937_read(&b, chunked + offset, len);
  }
  for (size_t i = 0; i < sizeof(whole); i++)
    assert(whole[i] == chunked[i]);
  mt19937_seed(&b, 0);
  mt19937_read(&b, chunked, 5);
  mt19937_seed(&b, 0);
  mt19937_read(&b, chunked, 16);
  for (size_t i = 0; i < 16; i++)
    assert(chunked[i] == whole[i]);
}
int main(void) {
  files_and_preopens();
  capabilities();
  descriptor_flags();
  symlink_metadata();
  random_stream();
  return 0;
}
