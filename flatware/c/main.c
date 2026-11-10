#include "fix.h"
#include "flatware.h"
#include "wasm.h"

__attribute__((import_module("flatware_control"), import_name("run"))) void
guest_run(void);
__attribute__((import_module("flatware_control"),
               import_name("input"))) fix_handle
current_input(void);
__attribute__((import_module("flatware_control"),
               import_name("set_input"))) void
set_input(fix_handle input);
WASM_MEMORY(1);
WASM_MEMORY_COPY(0, 1);
WASM_MEMORY_COPY(1, 0);

__attribute__((export_name("flatware_main_memory_size"))) uint32_t
main_memory_size(void) {
  return __builtin_wasm_memory_size(0);
}
__attribute__((export_name("flatware_scratch_memory_size"))) uint32_t
scratch_memory_size(void) {
  return wasm_memory_size_1();
}
static fix_handle force(fix_handle value) {
  while (!fix_is_object(value)) {
    fix_handle previous = value;
    switch (fix_get_type(value)) {
    case FIX_REF: {
      fix_handle thunk = fix_create_identification_thunk(value);
      value = fix_create_strict_encode(thunk);
      fix_close(thunk);
      break;
    }
    case FIX_THUNK:
      value = fix_create_strict_encode(value);
      break;
    case FIX_ENCODE:
      value = fix_clone(value);
      break;
    case FIX_OBJECT:
      __builtin_unreachable();
    }
    fix_close(previous);
    previous = value;
    value = fix_eval(value);
    fix_close(previous);
  }
  return value;
}
static fix_handle tree(fix_handle value, uint32_t length) {
  value = force(fix_clone(value));
  require(fix_is_object(value) && fix_get_data_type(value) == FIX_TREE &&
          fix_len(value) == length);
  return value;
}
static fix_handle entry(fix_handle value, uint32_t index) {
  value = force(fix_clone(value));
  fix_handle child;
  require(fix_is_object(value) && fix_get_data_type(value) == FIX_TREE);
  require(fix_access_tree(value, index, &child, 1) == 1);
  fix_close(value);
  return child;
}
static size_t local_length(fix_handle value) {
  value = force(fix_clone(value));
  uint64_t length = fix_len(value);
  fix_close(value);
  require(length <= SIZE_MAX);
  return (size_t)length;
}
static Bytes blob(fix_handle value) {
  value = force(fix_clone(value));
  require(fix_is_blob_obj(value));
  Bytes bytes = bytes_allocate(local_length(value));
  require(fix_access_blob(value, 0, bytes.data, bytes.len) == bytes.len);
  fix_close(value);
  return bytes;
}
static uint64_t integer(fix_handle value, uint32_t length) {
  value = force(fix_clone(value));
  require(fix_is_blob_obj(value) && fix_len(value) == length);
  uint8_t bytes[8];
  require(length <= sizeof(bytes));
  require(fix_access_blob(value, 0, bytes, length) == length);
  fix_close(value);
  return load_le(bytes, length);
}
static uint64_t integer_field(fix_handle parent, uint32_t index,
                              uint32_t length) {
  fix_handle value = entry(parent, index);
  uint64_t result = integer(value, length);
  fix_close(value);
  return result;
}
static fix_handle make_blob(const void *bytes, uint32_t length) {
  uint32_t pages = (length >> 16) + ((length & 65535) != 0),
           size = wasm_memory_size_1();
  if (pages > size)
    require(wasm_memory_grow_1(pages - size) >= 0);
  wasm_memory_copy_0_to_1(0, bytes, length);
  return fix_create_blob(1, length);
}
static fix_handle make_tree(fix_handle *values, uint32_t length) {
  fix_handle result = fix_create_tree(values, length);
  for (uint32_t i = 0; i < length; ++i)
    fix_close(values[i]);
  return result;
}
static Stat read_stat(fix_handle value) {
  value = tree(value, 7);
  Stat stat = {.dev = integer_field(value, 0, 8),
               .filetype = (uint8_t)integer_field(value, 1, 1),
               .nlink = integer_field(value, 2, 8),
               .size = integer_field(value, 3, 8),
               .atim = integer_field(value, 4, 8),
               .mtim = integer_field(value, 5, 8),
               .ctim = integer_field(value, 6, 8)};
  fix_close(value);
  return stat;
}
static fix_handle write_stat(const Stat *stat) {
  fix_handle values[] = {
      fix_create_blob_i64(stat->dev),   make_blob(&stat->filetype, 1),
      fix_create_blob_i64(stat->nlink), fix_create_blob_i64(stat->size),
      fix_create_blob_i64(stat->atim),  fix_create_blob_i64(stat->mtim),
      fix_create_blob_i64(stat->ctim)};
  return make_tree(values, 7);
}
static size_t read_node(Filesystem *fs, size_t parent, uint32_t input_index,
                        bool stream) {
  require(fs->count < SIZE_MAX);
  fs->nodes = reserve(fs->nodes, &fs->cap, fs->count + 1, sizeof(Node));
  size_t index = fs->count++;
  fs->nodes[index] = (Node){.parent = parent,
                            .input_parent = parent,
                            .input_index = input_index,
                            .stream = stream};
  return index;
}
static Bytes *byte_list(fix_handle value, size_t *count) {
  value = force(value);
  require(fix_is_object(value) && fix_get_data_type(value) == FIX_TREE);
  *count = local_length(value);
  require(*count <= SIZE_MAX / sizeof(Bytes));
  Bytes *list = malloc(*count * sizeof(Bytes));
  require(list || !*count);
  for (size_t i = 0; i < *count; ++i) {
    fix_handle child = entry(value, i);
    list[i] = blob(child);
    fix_close(child);
  }
  fix_close(value);
  return list;
}
static fix_handle original(fix_handle input, const Filesystem *fs,
                           size_t index) {
  const Node *node = &fs->nodes[index];
  require(node->input_index != NONE);
  fix_handle parent;
  if (node->input_parent == NONE) {
    parent = entry(input, 4);
  } else {
    fix_handle descriptor = original(input, fs, node->input_parent);
    parent = entry(descriptor, 2);
    fix_close(descriptor);
  }
  fix_handle result = entry(parent, node->input_index);
  fix_close(parent);
  return result;
}
void fs_load(Filesystem *fs, size_t index, unsigned fields) {
  fields &= ~fs->nodes[index].loaded;
  if (!fields)
    return;
  fix_handle descriptor = original(current_input(), fs, index);
  fix_handle checked = tree(descriptor, 3);
  fix_close(descriptor);
  descriptor = checked;
  if (fields & NODE_NAME) {
    fix_handle name = entry(descriptor, 0);
    fs->nodes[index].name = blob(name);
    fix_close(name);
    if (fs->nodes[index].parent != NONE) {
      const Bytes *name = &fs->nodes[index].name;
      require(name->len && !bytes_equal(name, (const uint8_t *)".", 1) &&
              !bytes_equal(name, (const uint8_t *)"..", 2));
      for (size_t i = 0; i < name->len; ++i)
        require(name->data[i] && name->data[i] != '/');
    }
  }
  if (fields & NODE_STAT) {
    Node *node = &fs->nodes[index];
    if (node->stream) {
      node->stat = (Stat){.filetype = 2,
                          .nlink = 1,
                          .atim = state.time,
                          .mtim = state.time,
                          .ctim = state.time};
    } else {
      fix_handle metadata = entry(descriptor, 1);
      node->stat = read_stat(metadata);
      fix_close(metadata);
    }
    node->stat.ino = index + 1;
    if (!node_file(node) || !node_directory(node)) {
      fix_handle contents = force(entry(descriptor, 2));
      node->stat.size = fix_len(contents);
      fix_close(contents);
    }
  }
  fs->nodes[index].loaded |= fields & (NODE_NAME | NODE_STAT);
  if (fields & NODE_CONTENT) {
    fs_load(fs, index, NODE_STAT);
    fix_handle contents = force(entry(descriptor, 2));
    if (!node_directory(&fs->nodes[index])) {
      require(fix_is_object(contents) &&
              fix_get_data_type(contents) == FIX_TREE);
      for (uint32_t i = 0; i < local_length(contents); ++i) {
        size_t child = read_node(fs, index, i, false);
        indices_push(&fs->nodes[index].children, child);
      }
    } else if (!node_file(&fs->nodes[index])) {
      fs->nodes[index].bytes = blob(contents);
    }
    fix_close(contents);
    fs->nodes[index].loaded |= NODE_CONTENT;
  }
  fix_close(descriptor);
}
static fix_handle write_node(fix_handle input, size_t index);
static fix_handle write_directory(fix_handle input, const Indices *children) {
  require(children->len <= SIZE_MAX / sizeof(fix_handle));
  fix_handle *values = malloc(children->len * sizeof(fix_handle));
  require(values || !children->len);
  for (size_t i = 0; i < children->len; ++i)
    values[i] = write_node(input, children->data[i]);
  fix_handle result = make_tree(values, children->len);
  free(values);
  return result;
}
static fix_handle write_node(fix_handle input, size_t index) {
  const Node *node = &state.fs.nodes[index];
  if (!node->changed)
    return original(input, &state.fs, index);
  fix_handle descriptor = node->input_index == NONE
                              ? fix_create_blob(0, 0)
                              : original(input, &state.fs, index);
  fix_handle name = node->changed & NODE_NAME
                        ? make_blob(node->name.data, node->name.len)
                        : entry(descriptor, 0);
  fix_handle metadata = !node->stream && (node->changed & NODE_STAT)
                            ? write_stat(&node->stat)
                            : entry(descriptor, 1);
  fix_handle contents;
  if (!(node->changed & NODE_CONTENT)) {
    contents = entry(descriptor, 2);
  } else {
    require(node->loaded & NODE_CONTENT);
    contents = !node_directory(node)
                   ? write_directory(input, &node->children)
                   : make_blob(node->bytes.data, node->bytes.len);
  }
  fix_handle fields[] = {name, metadata, contents};
  fix_close(descriptor);
  return make_tree(fields, 3);
}
__attribute__((export_name("_fix_apply"))) fix_handle apply(fix_handle input) {
  fix_handle checked = tree(input, 7);
  fix_close(input);
  input = checked;
  set_input(input);
  state = (State){0};
  state.time = integer_field(input, 6, 8);
  mt19937_seed(&state.random, integer_field(input, 5, 8));
  state.arguments = byte_list(entry(input, 2), &state.argument_count);
  size_t title_length = state.argument_count ? state.argument_count - 1 : 0;
  for (size_t i = 0; i < state.argument_count; ++i) {
    require(state.arguments[i].len <= SIZE_MAX - title_length);
    title_length += state.arguments[i].len;
  }
  Bytes title = bytes_allocate(title_length);
  size_t title_offset = 0;
  for (size_t i = 0; i < state.argument_count; ++i) {
    if (i)
      title.data[title_offset++] = ' ';
    memcpy(title.data + title_offset, state.arguments[i].data,
           state.arguments[i].len);
    title_offset += state.arguments[i].len;
  }
  fix_advise(input, "fix::title", 10, title.data, title.len);
  free(title.data);
  state.environment = byte_list(entry(input, 3), &state.environment_count);
  fix_handle descriptors = force(entry(input, 4));
  require(fix_is_object(descriptors) &&
          fix_get_data_type(descriptors) == FIX_TREE);
  for (uint32_t i = 0; i < local_length(descriptors); ++i)
    indices_push(&state.fs.roots, read_node(&state.fs, NONE, i, i < 3));
  fix_close(descriptors);
  fs_init(&state.fs);
  guest_run();
  fix_handle roots = write_directory(input, &state.fs.roots);
  fix_handle values[] = {roots, fix_create_blob_i32(state.exit_status)};
  fix_handle result = make_tree(values, 2);
  fix_detach_blob(1, 0);
  fs_destroy(&state.fs);
  set_input(0);
  fix_close(input);
  for (size_t i = 0; i < state.argument_count; ++i)
    free(state.arguments[i].data);
  for (size_t i = 0; i < state.environment_count; ++i)
    free(state.environment[i].data);
  free(state.arguments);
  free(state.environment);
  return result;
}
