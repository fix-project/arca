#include "flatware.h"
#define IMPORT(name)                                                           \
  __attribute__((import_module("flatware_control"), import_name(#name)))
IMPORT(read) void guest_read(uint32_t, void *, uint32_t);
IMPORT(write) void guest_write(uint32_t, const void *, uint32_t);
IMPORT(size) uint32_t guest_size(void);
IMPORT(exit) __attribute__((noreturn)) void guest_exit(void);
#undef IMPORT
#define WASI(name) __attribute__((export_name(#name))) uint32_t name
#define TRY(expression)                                                        \
  do {                                                                         \
    uint32_t error = (expression);                                             \
    if (error)                                                                 \
      return error;                                                            \
  } while (0)
#define BEGIN() TRY(tick())
State state;
static uint32_t tick(void) {
  if (state.time == UINT64_MAX)
    return OVERFLOW;
  state.time++;
  return 0;
}
static uint32_t bounds(uint32_t pointer, uint64_t len) {
  uint64_t size = (uint64_t)guest_size() * 65536;
  return pointer > size || len > size - pointer ? FAULT : 0;
}
static uint32_t load(uint32_t pointer, uint32_t len, Bytes *out) {
  TRY(bounds(pointer, len));
  *out = bytes_allocate(len);
  guest_read(pointer, out->data, len);
  return 0;
}
static uint32_t store(uint32_t pointer, const void *bytes, size_t len) {
  TRY(bounds(pointer, len));
  guest_write(pointer, bytes, (uint32_t)len);
  return 0;
}
static uint32_t store32(uint32_t p, uint32_t v) {
  uint8_t bytes[4];
  put_le(bytes, v, 4);
  return store(p, bytes, 4);
}
static uint32_t store64(uint32_t p, uint64_t v) {
  uint8_t bytes[8];
  put_le(bytes, v, 8);
  return store(p, bytes, 8);
}
static uint32_t strings_size(Bytes *strings, size_t count,
                             uint32_t output_count, uint32_t output_len) {
  TRY(bounds(output_count, 4));
  TRY(bounds(output_len, 4));
  if (count > UINT32_MAX)
    return OVERFLOW;
  uint32_t total = 0;
  for (size_t i = 0; i < count; i++) {
    if (strings[i].len >= UINT32_MAX || strings[i].len + 1 > UINT32_MAX - total)
      return OVERFLOW;
    total += (uint32_t)strings[i].len + 1;
  }
  TRY(store32(output_count, (uint32_t)count));
  return store32(output_len, total);
}
static uint32_t strings_get(Bytes *strings, size_t count, uint32_t pointers,
                            uint32_t buffer) {
  if (count > UINT32_MAX / 4)
    return OVERFLOW;
  TRY(bounds(pointers, count * 4));
  uint64_t len = 0;
  for (size_t i = 0; i < count; i++) {
    if (strings[i].len > UINT64_MAX - len - 1)
      return OVERFLOW;
    len += strings[i].len + 1;
    for (size_t j = 0; j < strings[i].len; j++)
      if (!strings[i].data[j])
        return INVAL;
  }
  TRY(bounds(buffer, len));
  uint32_t current = buffer;
  for (size_t i = 0; i < count; i++) {
    TRY(store32(pointers + (uint32_t)i * 4, current));
    TRY(store(current, strings[i].data, strings[i].len));
    current += (uint32_t)strings[i].len;
    uint8_t zero = 0;
    TRY(store(current++, &zero, 1));
  }
  return 0;
}
WASI(args_sizes_get)(uint32_t count, uint32_t bytes) {
  BEGIN();
  return strings_size(state.arguments, state.argument_count, count, bytes);
}
WASI(args_get)(uint32_t pointers, uint32_t buffer) {
  BEGIN();
  return strings_get(state.arguments, state.argument_count, pointers, buffer);
}
WASI(environ_sizes_get)(uint32_t count, uint32_t bytes) {
  BEGIN();
  return strings_size(state.environment, state.environment_count, count, bytes);
}
WASI(environ_get)(uint32_t pointers, uint32_t buffer) {
  BEGIN();
  return strings_get(state.environment, state.environment_count, pointers,
                     buffer);
}
static uint32_t vectors(uint32_t pointer, uint32_t count, Bytes *out,
                        size_t *total) {
  if (count > UINT32_MAX / 8)
    return OVERFLOW;
  TRY(load(pointer, count * 8, out));
  *total = 0;
  for (size_t i = 0; i < count; i++) {
    uint32_t p = (uint32_t)load_le(out->data + i * 8, 4),
             n = (uint32_t)load_le(out->data + i * 8 + 4, 4);
    uint32_t error = bounds(p, n);
    if (!error && n > UINT32_MAX - *total)
      error = OVERFLOW;
    if (error) {
      free(out->data);
      *out = (Bytes){0};
      return error;
    }
    *total += n;
  }
  return 0;
}
enum IoOperation { IO_READ, IO_WRITE };
static uint32_t transfer(uint32_t fd, uint32_t pointer, uint32_t count,
                         enum IoOperation operation, bool positioned,
                         uint64_t offset, uint32_t used) {
  TRY(bounds(used, 4));
  Bytes vectors_bytes = {0}, bytes = {0};
  size_t total;
  TRY(vectors(pointer, count, &vectors_bytes, &total));
  uint32_t error = operation == IO_WRITE
                       ? fs_write(&state.fs, fd, total, positioned, offset,
                                  state.time, &bytes)
                       : fs_read(&state.fs, fd, total, positioned, offset,
                                 state.time, &bytes);
  size_t cursor = 0;
  for (size_t i = 0; !error && i < count; i++) {
    uint32_t pointer = (uint32_t)load_le(vectors_bytes.data + i * 8, 4);
    size_t length = (uint32_t)load_le(vectors_bytes.data + i * 8 + 4, 4);
    if (length > bytes.len - cursor)
      length = bytes.len - cursor;
    uint8_t *buffer = length ? bytes.data + cursor : NULL;
    if (operation == IO_WRITE)
      guest_read(pointer, buffer, length);
    else
      error = store(pointer, buffer, length);
    cursor += length;
  }
  if (!error)
    error = store32(used, (uint32_t)bytes.len);
  free(vectors_bytes.data);
  return error;
}
WASI(fd_read)(uint32_t fd, uint32_t v, uint32_t n, uint32_t out) {
  BEGIN();
  return transfer(fd, v, n, IO_READ, false, 0, out);
}
WASI(fd_write)(uint32_t fd, uint32_t v, uint32_t n, uint32_t out) {
  BEGIN();
  return transfer(fd, v, n, IO_WRITE, false, 0, out);
}
WASI(fd_pread)(uint32_t fd, uint32_t v, uint32_t n, uint64_t offset,
               uint32_t out) {
  BEGIN();
  return transfer(fd, v, n, IO_READ, true, offset, out);
}
WASI(fd_pwrite)(uint32_t fd, uint32_t v, uint32_t n, uint64_t offset,
                uint32_t out) {
  BEGIN();
  return transfer(fd, v, n, IO_WRITE, true, offset, out);
}
WASI(fd_seek)(uint32_t fd, int64_t offset, uint32_t whence, uint32_t out) {
  BEGIN();
  TRY(bounds(out, 8));
  uint64_t value;
  TRY(fs_seek(&state.fs, fd, offset, whence, &value));
  return store64(out, value);
}
WASI(fd_tell)(uint32_t fd, uint32_t out) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, 0, &d));
  Node *n = &state.fs.nodes[d->node];
  TRY(node_file(n));
  if (n->stat.filetype == 2)
    return SPIPE;
  TRY(fs_descriptor(&state.fs, fd, TELL, &d));
  return store64(out, d->offset);
}
WASI(fd_close)(uint32_t fd) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, 0, &d));
  d->active = false;
  return 0;
}
WASI(fd_fdstat_get)(uint32_t fd, uint32_t out) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, 0, &d));
  uint8_t bytes[24] = {0};
  bytes[0] = state.fs.nodes[d->node].stat.filetype;
  put_le(bytes + 2, d->flags, 2);
  put_le(bytes + 8, d->rights, 8);
  put_le(bytes + 16, d->inheriting, 8);
  return store(out, bytes, 24);
}
WASI(fd_fdstat_set_rights)(uint32_t fd, uint64_t rights, uint64_t inheriting) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, 0, &d));
  if ((rights & ~d->rights) || (inheriting & ~d->inheriting))
    return NOTCAPABLE;
  d->rights = rights;
  d->inheriting = inheriting;
  return 0;
}
WASI(fd_fdstat_set_flags)(uint32_t fd, uint32_t flags) {
  BEGIN();
  return fs_set_flags(&state.fs, fd, flags);
}

static uint32_t node_stat(size_t node, uint32_t out) {
  Stat stat = state.fs.nodes[node].stat;
  stat.size = node_size(&state.fs.nodes[node]);
  uint8_t bytes[64];
  stat_encode(&stat, bytes);
  return store(out, bytes, 64);
}
WASI(fd_filestat_get)(uint32_t fd, uint32_t out) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, RIGHT(21), &d));
  return node_stat(d->node, out);
}
WASI(fd_filestat_set_size)(uint32_t fd, uint64_t size) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, RIGHT(22), &d));
  return fs_resize(&state.fs, d->node, size, state.time);
}
static uint32_t set_times(Stat *s, uint64_t atim, uint64_t mtim,
                          uint32_t flags) {
  if ((flags & ~15u) || (flags & 3) == 3 || (flags & 12) == 12)
    return INVAL;
  if (flags & 1)
    s->atim = atim;
  if (flags & 2)
    s->atim = state.time;
  if (flags & 4)
    s->mtim = mtim;
  if (flags & 8)
    s->mtim = state.time;
  s->ctim = state.time;
  return 0;
}
WASI(fd_filestat_set_times)(uint32_t fd, uint64_t atim, uint64_t mtim,
                            uint32_t flags) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, RIGHT(23), &d));
  return set_times(&state.fs.nodes[d->node].stat, atim, mtim, flags);
}
WASI(fd_prestat_get)(uint32_t fd, uint32_t out) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, 0, &d));
  if (!d->preopen)
    return BADF;
  uint8_t bytes[8] = {0};
  put_le(bytes + 4, state.fs.nodes[d->node].name.len, 4);
  return store(out, bytes, 8);
}
WASI(fd_prestat_dir_name)(uint32_t fd, uint32_t out, uint32_t length) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, 0, &d));
  if (!d->preopen)
    return BADF;
  Bytes *name = &state.fs.nodes[d->node].name;
  if (length < name->len)
    return 37;
  return store(out, name->data, name->len);
}
WASI(path_open)(uint32_t fd, uint32_t lookup, uint32_t pointer, uint32_t length,
                uint32_t oflags, uint64_t rights, uint64_t inheriting,
                uint32_t flags, uint32_t out) {
  BEGIN();
  if (lookup & ~1u)
    return INVAL;
  TRY(bounds(out, 4));
  Bytes path = {0};
  TRY(load(pointer, length, &path));
  uint32_t opened, error = fs_open(&state.fs, fd, &path, oflags, rights,
                                   inheriting, flags, state.time, &opened);
  free(path.data);
  if (error)
    return error;
  return store32(out, opened);
}
static uint32_t resolve_path(uint32_t fd, uint32_t lookup, uint32_t pointer,
                             uint32_t length, uint64_t rights, size_t *node) {
  if (lookup & ~1u)
    return INVAL;
  Descriptor *descriptor;
  TRY(fs_descriptor(&state.fs, fd, rights, &descriptor));
  Bytes path = {0};
  TRY(load(pointer, length, &path));
  uint32_t error = fs_resolve(&state.fs, fd, &path, node);
  free(path.data);
  if (error)
    return error;
  return (lookup & 1) && state.fs.nodes[*node].stat.filetype == 7 ? NOTSUP : 0;
}
WASI(path_filestat_get)(uint32_t fd, uint32_t flags, uint32_t pointer,
                        uint32_t len, uint32_t out) {
  BEGIN();
  size_t node;
  TRY(resolve_path(fd, flags, pointer, len, RIGHT(18), &node));
  return node_stat(node, out);
}
WASI(path_filestat_set_times)(uint32_t fd, uint32_t lookup, uint32_t pointer,
                              uint32_t len, uint64_t atim, uint64_t mtim,
                              uint32_t flags) {
  BEGIN();
  size_t node;
  TRY(resolve_path(fd, lookup, pointer, len, RIGHT(20), &node));
  return set_times(&state.fs.nodes[node].stat, atim, mtim, flags);
}
WASI(path_create_directory)(uint32_t fd, uint32_t pointer, uint32_t len) {
  BEGIN();
  Bytes path = {0};
  TRY(load(pointer, len, &path));
  uint32_t error = fs_mkdir(&state.fs, fd, &path, state.time);
  free(path.data);
  return error;
}
static uint32_t unlink_path(uint32_t fd, uint32_t p, uint32_t len,
                            bool directory) {
  Bytes path = {0};
  TRY(load(p, len, &path));
  uint32_t error = fs_unlink(&state.fs, fd, &path, directory, state.time);
  free(path.data);
  return error;
}
WASI(path_unlink_file)(uint32_t fd, uint32_t p, uint32_t len) {
  BEGIN();
  return unlink_path(fd, p, len, false);
}
WASI(path_remove_directory)(uint32_t fd, uint32_t p, uint32_t len) {
  BEGIN();
  return unlink_path(fd, p, len, true);
}
WASI(path_rename)(uint32_t old_fd, uint32_t old_pointer, uint32_t old_len,
                  uint32_t new_fd, uint32_t new_pointer, uint32_t new_len) {
  BEGIN();
  Bytes old_path = {0}, new_path = {0};
  TRY(load(old_pointer, old_len, &old_path));
  uint32_t error = load(new_pointer, new_len, &new_path);
  if (!error)
    error =
        fs_rename(&state.fs, old_fd, &old_path, new_fd, &new_path, state.time);
  free(old_path.data);
  free(new_path.data);
  return error;
}
WASI(fd_readdir)(uint32_t fd, uint32_t out, uint32_t length, uint64_t cookie,
                 uint32_t used) {
  BEGIN();
  TRY(bounds(out, length));
  TRY(bounds(used, 4));
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, RIGHT(14), &d));
  Node *directory = &state.fs.nodes[d->node];
  TRY(node_directory(directory));
  if (cookie > directory->children.len)
    return INVAL;
  size_t cursor = 0;
  for (size_t i = (size_t)cookie;
       i < directory->children.len && cursor < length; i++) {
    Node *n = &state.fs.nodes[directory->children.data[i]];
    uint8_t header[24] = {0};
    put_le(header, i + 1, 8);
    put_le(header + 8, n->stat.ino, 8);
    put_le(header + 16, n->name.len, 4);
    header[20] = n->stat.filetype;
    size_t count = 24 < length - cursor ? 24 : length - cursor;
    TRY(store(out + (uint32_t)cursor, header, count));
    cursor += count;
    count = n->name.len < length - cursor ? n->name.len : length - cursor;
    TRY(store(out + (uint32_t)cursor, n->name.data, count));
    cursor += count;
  }
  return store32(used, (uint32_t)cursor);
}
WASI(fd_renumber)(uint32_t from, uint32_t to) {
  BEGIN();
  Filesystem *fs = &state.fs;
  Descriptor *source;
  TRY(fs_descriptor(fs, from, 0, &source));
  if (from == to)
    return 0;
  Descriptor moved = *source;
  if ((uint64_t)to + 1 > SIZE_MAX / sizeof(Descriptor))
    return OVERFLOW;
  size_t count = (size_t)to + 1;
  if (count > fs->descriptor_count) {
    fs->descriptors = reserve(fs->descriptors, &fs->descriptor_cap, count,
                              sizeof(Descriptor));
    memset(fs->descriptors + fs->descriptor_count, 0,
           (count - fs->descriptor_count) * sizeof(Descriptor));
    fs->descriptor_count = count;
  }
  fs->descriptors[to] = moved;
  fs->descriptors[from].active = false;
  return 0;
}
WASI(fd_sync)(uint32_t fd) {
  BEGIN();
  Descriptor *d;
  return fs_descriptor(&state.fs, fd, RIGHT(4), &d);
}
WASI(fd_datasync)(uint32_t fd) {
  BEGIN();
  Descriptor *d;
  return fs_descriptor(&state.fs, fd, RIGHT(0), &d);
}
WASI(fd_advise)(uint32_t fd, uint64_t offset __attribute__((unused)),
                uint64_t length __attribute__((unused)), uint32_t advice) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, RIGHT(7), &d));
  return advice > 5 ? INVAL : 0;
}
WASI(fd_allocate)(uint32_t fd, uint64_t offset, uint64_t length) {
  BEGIN();
  Descriptor *d;
  TRY(fs_descriptor(&state.fs, fd, RIGHT(8), &d));
  Node *node = &state.fs.nodes[d->node];
  TRY(node_file(node));
  if (length > UINT64_MAX - offset)
    return OVERFLOW;
  uint64_t end = offset + length;
  return end > node_size(node) ? fs_resize(&state.fs, d->node, end, state.time)
                               : 0;
}
WASI(random_get)(uint32_t p, uint32_t len) {
  BEGIN();
  TRY(bounds(p, len));
  Bytes bytes = bytes_allocate(len);
  mt19937_read(&state.random, bytes.data, len);
  uint32_t error = store(p, bytes.data, len);
  free(bytes.data);
  return error;
}
WASI(clock_res_get)(uint32_t clock, uint32_t out) {
  BEGIN();
  if (clock > 3)
    return INVAL;
  return store64(out, 1);
}
WASI(clock_time_get)(uint32_t clock, uint64_t precision __attribute__((unused)),
                     uint32_t out) {
  BEGIN();
  if (clock > 3)
    return INVAL;
  return store64(out, state.time);
}
WASI(sched_yield)(void) {
  BEGIN();
  return 0;
}
__attribute__((export_name("proc_exit"), noreturn)) void
proc_exit(uint32_t status) {
  require(tick() == 0);
  state.exit_status = status;
  guest_exit();
}
#define UNUSED __attribute__((unused))
#define UNSUPPORTED(name, args)                                                \
  WASI(name) args {                                                            \
    BEGIN();                                                                   \
    return 52;                                                                 \
  }
UNSUPPORTED(path_link, (uint32_t a UNUSED, uint32_t b UNUSED, uint32_t c UNUSED,
                        uint32_t d UNUSED, uint32_t e UNUSED, uint32_t f UNUSED,
                        uint32_t g UNUSED))
UNSUPPORTED(path_readlink,
            (uint32_t a UNUSED, uint32_t b UNUSED, uint32_t c UNUSED,
             uint32_t d UNUSED, uint32_t e UNUSED, uint32_t f UNUSED))
UNSUPPORTED(path_symlink,
            (uint32_t a UNUSED, uint32_t b UNUSED, uint32_t c UNUSED,
             uint32_t d UNUSED, uint32_t e UNUSED))
UNSUPPORTED(poll_oneoff, (uint32_t a UNUSED, uint32_t b UNUSED,
                          uint32_t c UNUSED, uint32_t d UNUSED))
UNSUPPORTED(proc_raise, (uint32_t a UNUSED))
UNSUPPORTED(sock_accept,
            (uint32_t a UNUSED, uint32_t b UNUSED, uint32_t c UNUSED))
UNSUPPORTED(sock_recv,
            (uint32_t a UNUSED, uint32_t b UNUSED, uint32_t c UNUSED,
             uint32_t d UNUSED, uint32_t e UNUSED, uint32_t f UNUSED))
UNSUPPORTED(sock_send, (uint32_t a UNUSED, uint32_t b UNUSED, uint32_t c UNUSED,
                        uint32_t d UNUSED, uint32_t e UNUSED))
UNSUPPORTED(sock_shutdown, (uint32_t a UNUSED, uint32_t b UNUSED))
