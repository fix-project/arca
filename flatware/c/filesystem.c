#include "flatware.h"
#define TRY(expression)                                                        \
  do {                                                                         \
    uint32_t error = (expression);                                             \
    if (error)                                                                 \
      return error;                                                            \
  } while (0)
uint64_t node_size(const Node *n) {
  if (!(n->loaded & NODE_CONTENT))
    return n->stat.size;
  return !node_file(n)        ? n->bytes.len
         : !node_directory(n) ? n->children.len
                              : n->stat.size;
}
void fs_changed(Filesystem *fs, size_t index, unsigned fields) {
  fs->nodes[index].changed |= fields;
  for (size_t parent = fs->nodes[index].parent; parent != NONE;
       parent = fs->nodes[parent].parent)
    fs->nodes[parent].changed |= NODE_CONTENT;
}
void node_modified(Filesystem *fs, size_t index, uint64_t time) {
  Node *n = &fs->nodes[index];
  n->stat.size = node_size(n);
  if (!n->stream)
    n->stat.mtim = n->stat.ctim = time;
  fs_changed(fs, index, NODE_STAT | NODE_CONTENT);
}
uint32_t node_file(const Node *n) {
  if (n->stat.dev)
    return NOTSUP;
  return n->stat.filetype == 3                ? ISDIR
         : n->stat.filetype == 4 || n->stream ? 0
                                              : NOTSUP;
}
uint32_t node_directory(const Node *n) {
  if (n->stat.dev)
    return NOTSUP;
  return n->stat.filetype == 3                ? 0
         : n->stat.filetype == 4 || n->stream ? NOTDIR
                                              : NOTSUP;
}
void stat_encode(const Stat *s, uint8_t output[64]) {
  memset(output, 0, 64);
  put_le(output, s->dev, 8);
  put_le(output + 8, s->ino, 8);
  output[16] = s->filetype;
  uint64_t fields[] = {s->nlink, s->size, s->atim, s->mtim, s->ctim};
  for (size_t i = 0; i < 5; i++)
    put_le(output + 24 + i * 8, fields[i], 8);
}
void fs_init(Filesystem *fs) {
  require(fs->roots.len >= 3);
  fs->descriptors = reserve(fs->descriptors, &fs->descriptor_cap, fs->roots.len,
                            sizeof(Descriptor));
  fs->descriptor_count = fs->roots.len;
  for (size_t fd = 0; fd < fs->roots.len; fd++) {
    size_t index = fs->roots.data[fd];
    require(index < fs->count);
    fs_load(fs, index, NODE_NAME | NODE_STAT);
    Node *node = &fs->nodes[index];
    bool directory = node->stat.filetype == 3;
    require(fd >= 3 || !directory);
    uint64_t rights = fd == 0 ? READ | RIGHT(3) | RIGHT(21)
                      : fd < 3
                          ? WRITE | RIGHT(0) | RIGHT(3) | RIGHT(4) | RIGHT(21)
                      : directory ? DIRECTORY_RIGHTS
                                  : FILE_RIGHTS;
    fs->descriptors[fd] = (Descriptor){
        .node = index,
        .offset = fd == 1 || fd == 2 ? node_size(node) : 0,
        .rights = rights,
        .inheriting = directory ? FILE_RIGHTS | DIRECTORY_RIGHTS : 0,
        .flags = fd == 1 || fd == 2 ? 1 : 0,
        .preopen = fd >= 3 && directory,
        .active = true};
  }
}
void fs_destroy(Filesystem *fs) {
  for (size_t i = 0; i < fs->count; i++) {
    free(fs->nodes[i].name.data);
    free(fs->nodes[i].bytes.data);
    free(fs->nodes[i].children.data);
  }
  free(fs->nodes);
  free(fs->roots.data);
  free(fs->descriptors);
  memset(fs, 0, sizeof(*fs));
}
uint32_t fs_descriptor(Filesystem *fs, uint32_t fd, uint64_t rights,
                       Descriptor **result) {
  if (fd >= fs->descriptor_count || !fs->descriptors[fd].active)
    return BADF;
  Descriptor *d = &fs->descriptors[fd];
  if ((d->rights & rights) != rights)
    return NOTCAPABLE;
  fs_load(fs, d->node, NODE_STAT);
  *result = d;
  return 0;
}
uint32_t fs_set_flags(Filesystem *fs, uint32_t fd, uint32_t flags) {
  Descriptor *d;
  TRY(fs_descriptor(fs, fd, RIGHT(3), &d));
  if (flags & ~31u)
    return INVAL;
  d->flags = (uint16_t)flags;
  return 0;
}
static size_t find_child(Filesystem *fs, size_t parent, const uint8_t *name,
                         size_t len) {
  fs_load(fs, parent, NODE_CONTENT);
  Indices *entries = &fs->nodes[parent].children;
  for (size_t i = 0; i < entries->len; i++) {
    size_t child = entries->data[i];
    fs_load(fs, child, NODE_NAME);
    if (bytes_equal(&fs->nodes[child].name, name, len))
      return child;
  }
  return NONE;
}
uint32_t fs_resolve(Filesystem *fs, uint32_t fd, const Bytes *path,
                    size_t *output) {
  Descriptor *d;
  TRY(fs_descriptor(fs, fd, 0, &d));
  size_t root = d->node, node = root;
  TRY(node_directory(&fs->nodes[root]));
  if (!path->len)
    return INVAL;
  for (size_t i = 0; i < path->len; i++)
    if (!path->data[i])
      return INVAL;
  if (path->data[0] == '/')
    return NOTCAPABLE;
  for (size_t start = 0; start < path->len;) {
    size_t end = start;
    while (end < path->len && path->data[end] != '/')
      end++;
    size_t len = end - start;
    if (len) {
      TRY(node_directory(&fs->nodes[node]));
      const uint8_t *part = path->data + start;
      if (len == 1 && part[0] == '.') {
      } else if (len == 2 && part[0] == '.' && part[1] == '.') {
        if (node == root || fs->nodes[node].parent == NONE)
          return NOTCAPABLE;
        node = fs->nodes[node].parent;
      } else {
        node = find_child(fs, node, part, len);
        if (node == NONE)
          return NOENT;
      }
    }
    fs_load(fs, node, NODE_STAT);
    start = end < path->len ? end + 1 : end;
  }
  if (path->data[path->len - 1] == '/')
    TRY(node_directory(&fs->nodes[node]));
  *output = node;
  return 0;
}
static uint32_t path_parent(Filesystem *fs, uint32_t fd, const Bytes *path,
                            size_t *parent, Bytes *name) {
  if (!path->len)
    return INVAL;
  if (path->data[0] == '/')
    return NOTCAPABLE;
  size_t end = path->len;
  while (end && path->data[end - 1] == '/')
    end--;
  size_t split = end;
  while (split && path->data[split - 1] != '/')
    split--;
  size_t len = end - split;
  const uint8_t *part = path->data + split;
  if (!len || (len == 1 && part[0] == '.') ||
      (len == 2 && part[0] == '.' && part[1] == '.'))
    return INVAL;
  for (size_t i = 0; i < len; i++)
    if (!part[i])
      return INVAL;
  Bytes prefix = split ? (Bytes){.data = path->data, .len = split - 1}
                       : (Bytes){.data = (uint8_t *)".", .len = 1};
  TRY(fs_resolve(fs, fd, &prefix, parent));
  TRY(node_directory(&fs->nodes[*parent]));
  *name = (Bytes){.data = (uint8_t *)part, .len = len};
  return 0;
}
static uint32_t create(Filesystem *fs, size_t parent, const Bytes *name,
                       bool directory, uint64_t time, size_t *out) {
  TRY(node_directory(&fs->nodes[parent]));
  if (find_child(fs, parent, name->data, name->len) != NONE)
    return EXIST;
  require(fs->count < SIZE_MAX);
  fs->nodes = reserve(fs->nodes, &fs->cap, fs->count + 1, sizeof(Node));
  size_t index = fs->count++;
  fs->nodes[index] = (Node){.name = bytes_copy(name->data, name->len),
                            .stat = {.ino = index + 1,
                                     .nlink = 1,
                                     .atim = time,
                                     .mtim = time,
                                     .ctim = time,
                                     .filetype = directory ? 3 : 4},
                            .parent = parent,
                            .input_parent = NONE,
                            .input_index = NONE,
                            .loaded = NODE_ALL,
                            .changed = NODE_ALL};
  indices_push(&fs->nodes[parent].children, index);
  node_modified(fs, parent, time);
  *out = index;
  return 0;
}
uint32_t fs_open(Filesystem *fs, uint32_t fd, const Bytes *path,
                 uint32_t oflags, uint64_t rights, uint64_t inheriting,
                 uint32_t flags, uint64_t time, uint32_t *out) {
  if ((oflags & ~15u) || (flags & ~31u))
    return INVAL;
  Descriptor *parent;
  TRY(fs_descriptor(fs, fd, RIGHT(13), &parent));
  if ((rights | inheriting) & ~parent->inheriting)
    return NOTCAPABLE;
  if (oflags & 8)
    TRY(fs_descriptor(fs, fd, RIGHT(19), &parent));
  size_t node;
  uint32_t error = fs_resolve(fs, fd, path, &node);
  if (!error) {
    if ((oflags & 5) == 5)
      return EXIST;
  } else if (error == NOENT && (oflags & 1)) {
    TRY(fs_descriptor(fs, fd, RIGHT(10), &parent));
    if ((oflags & 2) || path->data[path->len - 1] == '/')
      return NOTDIR;
    size_t directory;
    Bytes name = {0};
    TRY(path_parent(fs, fd, path, &directory, &name));
    error = create(fs, directory, &name, false, time, &node);
    if (error)
      return error;
  } else
    return error;
  bool directory = !node_directory(&fs->nodes[node]);
  if (!directory && node_file(&fs->nodes[node]))
    return NOTSUP;
  if ((oflags & 2) && !directory)
    return NOTDIR;
  if (oflags & 8) {
    if (directory)
      return ISDIR;
    bytes_resize(&fs->nodes[node].bytes, 0);
    fs->nodes[node].loaded |= NODE_CONTENT;
    node_modified(fs, node, time);
  }
  size_t slot = 0;
  while (slot < fs->descriptor_count && fs->descriptors[slot].active)
    slot++;
  if (slot > UINT32_MAX)
    return OVERFLOW;
  if (slot == fs->descriptor_count) {
    fs->descriptors = reserve(fs->descriptors, &fs->descriptor_cap, slot + 1,
                              sizeof(Descriptor));
    fs->descriptor_count++;
  }
  fs->descriptors[slot] = (Descriptor){
      .node = node,
      .rights = rights & (directory ? DIRECTORY_RIGHTS : FILE_RIGHTS),
      .inheriting = directory ? inheriting : 0,
      .flags = (uint16_t)flags,
      .active = true};
  *out = (uint32_t)slot;
  return 0;
}
uint32_t fs_read(Filesystem *fs, uint32_t fd, size_t count, bool positioned,
                 uint64_t offset, uint64_t time, Bytes *out) {
  Descriptor *d;
  TRY(fs_descriptor(fs, fd, READ | (positioned ? SEEK : 0), &d));
  Node *n = &fs->nodes[d->node];
  if (positioned && n->stat.filetype == 2)
    return SPIPE;
  TRY(node_file(n));
  fs_load(fs, d->node, NODE_CONTENT);
  n = &fs->nodes[d->node];
  uint64_t position = positioned ? offset : d->offset;
  size_t start = position < n->bytes.len ? (size_t)position : n->bytes.len;
  size_t len = count < n->bytes.len - start ? count : n->bytes.len - start;
  if (!positioned && len > UINT64_MAX - position)
    return OVERFLOW;
  *out = (Bytes){.data = len ? n->bytes.data + start : NULL, .len = len};
  if (!positioned)
    d->offset = position + len;
  if (!n->stream) {
    n->stat.atim = time;
    fs_changed(fs, d->node, NODE_STAT);
  }
  return 0;
}
uint32_t fs_write(Filesystem *fs, uint32_t fd, size_t count, bool positioned,
                  uint64_t offset, uint64_t time, Bytes *out) {
  Descriptor *d;
  TRY(fs_descriptor(fs, fd, WRITE | (positioned ? SEEK : 0), &d));
  Node *n = &fs->nodes[d->node];
  if (positioned && n->stat.filetype == 2)
    return SPIPE;
  uint64_t position = !positioned && (d->flags & 1) ? node_size(n)
                      : positioned                  ? offset
                                                    : d->offset;
  if (position > SIZE_MAX || count > SIZE_MAX - (size_t)position)
    return OVERFLOW;
  TRY(node_file(n));
  *out = (Bytes){0};
  if (!count)
    return 0;
  fs_load(fs, d->node, NODE_CONTENT);
  n = &fs->nodes[d->node];
  size_t end = (size_t)position + count;
  if (end > n->bytes.len) {
    n->bytes.data = reserve(n->bytes.data, &n->bytes.cap, end, 1);
    if (position > n->bytes.len)
      memset(n->bytes.data + n->bytes.len, 0, (size_t)position - n->bytes.len);
    n->bytes.len = end;
  }
  *out = (Bytes){.data = n->bytes.data + (size_t)position, .len = count};
  node_modified(fs, d->node, time);
  if (!positioned)
    d->offset = end;
  return 0;
}
uint32_t fs_seek(Filesystem *fs, uint32_t fd, int64_t delta, uint32_t whence,
                 uint64_t *output) {
  Descriptor *d;
  TRY(fs_descriptor(fs, fd, 0, &d));
  Node *n = &fs->nodes[d->node];
  TRY(node_file(n));
  if (n->stat.filetype == 2)
    return SPIPE;
  TRY(fs_descriptor(fs, fd, SEEK, &d));
  if (whence > 2)
    return INVAL;
  uint64_t base = whence == 0   ? 0
                  : whence == 1 ? d->offset
                                : node_size(n),
           offset;
  if (delta < 0) {
    uint64_t magnitude = (uint64_t)(-(delta + 1)) + 1;
    if (magnitude > base)
      return INVAL;
    offset = base - magnitude;
  } else {
    if ((uint64_t)delta > UINT64_MAX - base)
      return INVAL;
    offset = base + (uint64_t)delta;
  }
  d->offset = offset;
  *output = offset;
  return 0;
}
uint32_t fs_resize(Filesystem *fs, size_t node, uint64_t size, uint64_t time) {
  if (size > SIZE_MAX)
    return OVERFLOW;
  TRY(node_file(&fs->nodes[node]));
  if (size)
    fs_load(fs, node, NODE_CONTENT);
  bytes_resize(&fs->nodes[node].bytes, (size_t)size);
  fs->nodes[node].loaded |= NODE_CONTENT;
  node_modified(fs, node, time);
  return 0;
}
uint32_t fs_mkdir(Filesystem *fs, uint32_t fd, const Bytes *path,
                  uint64_t time) {
  Descriptor *d;
  TRY(fs_descriptor(fs, fd, RIGHT(9), &d));
  size_t parent, node;
  Bytes name = {0};
  TRY(path_parent(fs, fd, path, &parent, &name));
  uint32_t e = create(fs, parent, &name, true, time, &node);
  return e;
}
static void remove_child(Node *parent, size_t child) {
  for (size_t i = 0; i < parent->children.len; i++)
    if (parent->children.data[i] == child) {
      memmove(parent->children.data + i, parent->children.data + i + 1,
              (parent->children.len - i - 1) * sizeof(size_t));
      parent->children.len--;
      return;
    }
}
uint32_t fs_unlink(Filesystem *fs, uint32_t fd, const Bytes *path,
                   bool directory, uint64_t time) {
  Descriptor *d;
  TRY(fs_descriptor(fs, fd, RIGHT(directory ? 25 : 26), &d));
  size_t root = d->node, node;
  TRY(fs_resolve(fs, fd, path, &node));
  if (node == root || fs->nodes[node].parent == NONE)
    return NOTCAPABLE;
  Node *n = &fs->nodes[node];
  if (!node_directory(n)) {
    if (!directory)
      return ISDIR;
    if (node_size(n))
      return NOTEMPTY;
  } else if (directory)
    return NOTDIR;
  size_t parent = n->parent;
  remove_child(&fs->nodes[parent], node);
  node_modified(fs, parent, time);
  n->stat.nlink = 0;
  n->stat.ctim = time;
  fs_changed(fs, node, NODE_STAT);
  return 0;
}
uint32_t fs_rename(Filesystem *fs, uint32_t old_fd, const Bytes *old_path,
                   uint32_t new_fd, const Bytes *new_path, uint64_t time) {
  Descriptor *d;
  TRY(fs_descriptor(fs, old_fd, RIGHT(16), &d));
  size_t root = d->node;
  TRY(fs_descriptor(fs, new_fd, RIGHT(17), &d));
  size_t node;
  TRY(fs_resolve(fs, old_fd, old_path, &node));
  if (new_path->len && new_path->data[new_path->len - 1] == '/')
    TRY(node_directory(&fs->nodes[node]));
  if (node == root || fs->nodes[node].parent == NONE)
    return NOTCAPABLE;
  size_t old_parent = fs->nodes[node].parent, new_parent;
  Bytes name = {0};
  TRY(path_parent(fs, new_fd, new_path, &new_parent, &name));
  for (size_t ancestor = new_parent; ancestor != NONE;
       ancestor = fs->nodes[ancestor].parent)
    if (ancestor == node) {
      return INVAL;
    }
  size_t existing = find_child(fs, new_parent, name.data, name.len);
  if (existing == node) {
    return 0;
  }
  if (existing != NONE) {
    bool directory = !node_directory(&fs->nodes[node]);
    fs_load(fs, existing, NODE_STAT);
    Node *replaced = &fs->nodes[existing];
    uint32_t e = 0;
    if (!node_directory(replaced))
      e = !directory ? ISDIR : node_size(replaced) ? NOTEMPTY : 0;
    else if (directory)
      e = NOTDIR;
    if (e) {
      return e;
    }
    remove_child(&fs->nodes[new_parent], existing);
    replaced->stat.nlink = 0;
    replaced->stat.ctim = time;
  }
  remove_child(&fs->nodes[old_parent], node);
  indices_push(&fs->nodes[new_parent].children, node);
  free(fs->nodes[node].name.data);
  fs->nodes[node].name = bytes_copy(name.data, name.len);
  fs->nodes[node].parent = new_parent;
  fs->nodes[node].stat.ctim = time;
  fs_changed(fs, node, NODE_NAME | NODE_STAT);
  node_modified(fs, old_parent, time);
  node_modified(fs, new_parent, time);
  return 0;
}
