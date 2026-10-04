#include "fix.h"
#include "flatware.h"
#include "wasm.h"

__attribute__((import_module("flatware_control"), import_name("run"))) void
guest_run(void);
WASM_MEMORY(1);
WASM_MEMORY_COPY(0, 1);
WASM_MEMORY_COPY(1, 0);
WASM_TABLE(0);

__attribute__((export_name("flatware_main_memory_size"))) uint32_t
main_memory_size(void) {
  return __builtin_wasm_memory_size(0);
}
__attribute__((export_name("flatware_scratch_memory_size"))) uint32_t
scratch_memory_size(void) {
  return wasm_memory_size_1();
}
__attribute__((export_name("flatware_read_table_size"))) uint32_t
read_table_size(void) {
  return wasm_table_size_0();
}
static fix_handle null(void) { return __builtin_wasm_ref_null_extern(); }
static void tree(fix_handle value, uint32_t length) {
  require(fix_is_object(value) && fix_get_data_type(value) == FIX_TREE &&
          fix_len(value) == length);
}
static fix_handle entry(fix_handle value, uint32_t index) {
  require(fix_is_object(value) && fix_get_data_type(value) == FIX_TREE &&
          index < fix_len(value));
  fix_attach_tree(value, 0);
  fix_handle child = wasm_table_get_0(index);
  fix_detach_tree(0, 0);
  return child;
}
static Bytes blob(fix_handle value) {
  require(fix_is_blob_obj(value));
  Bytes bytes = bytes_allocate(fix_len(value));
  fix_attach_blob(value, 1);
  wasm_memory_copy_1_to_0(bytes.data, 0, bytes.len);
  fix_detach_blob(1, 0);
  return bytes;
}
static uint64_t integer(fix_handle value, uint32_t length) {
  require(fix_is_blob_obj(value) && fix_len(value) == length);
  uint8_t bytes[8];
  require(length <= sizeof(bytes));
  fix_attach_blob(value, 1);
  wasm_memory_copy_1_to_0(bytes, 0, length);
  fix_detach_blob(1, 0);
  return load_le(bytes, length);
}
static fix_handle make_blob(const void *bytes, uint32_t length) {
  uint32_t pages = (length >> 16) + ((length & 65535) != 0),
           size = wasm_memory_size_1();
  if (pages > size)
    require(wasm_memory_grow_1(pages - size) >= 0);
  wasm_memory_copy_0_to_1(0, bytes, length);
  return fix_create_blob(1, length);
}
static void prepare_tree(uint32_t length) {
  uint32_t size = wasm_table_size_0();
  if (length > size)
    require(wasm_table_grow_0(null(), length - size) >= 0);
}
static fix_handle make_tree(uint32_t length) {
  fix_handle value = fix_create_tree(0, length);
  wasm_table_fill_0(0, null(), length);
  return value;
}
static Stat read_stat(fix_handle value) {
  tree(value, 8);
  Stat stat = {
      integer(entry(value, 0), 8),          integer(entry(value, 1), 8),
      (uint8_t)integer(entry(value, 2), 1), integer(entry(value, 3), 8),
      integer(entry(value, 4), 8),          integer(entry(value, 5), 8),
      integer(entry(value, 6), 8),          integer(entry(value, 7), 8)};
  return stat;
}
static fix_handle write_stat(const Stat *stat) {
  prepare_tree(8);
  wasm_table_set_0(0, fix_create_blob_i64(stat->dev));
  wasm_table_set_0(1, fix_create_blob_i64(stat->ino));
  wasm_table_set_0(2, make_blob(&stat->filetype, 1));
  wasm_table_set_0(3, fix_create_blob_i64(stat->nlink));
  wasm_table_set_0(4, fix_create_blob_i64(stat->size));
  wasm_table_set_0(5, fix_create_blob_i64(stat->atim));
  wasm_table_set_0(6, fix_create_blob_i64(stat->mtim));
  wasm_table_set_0(7, fix_create_blob_i64(stat->ctim));
  return make_tree(8);
}
static size_t read_node(fix_handle descriptor, size_t parent,
                        uint32_t input_index, bool stream) {
  tree(descriptor, 3);
  fix_handle name = entry(descriptor, 0), metadata = entry(descriptor, 1),
             contents = entry(descriptor, 2);
  Filesystem *fs = &state.fs;
  Stat stat = stream ? (Stat){.ino = fs->count,
                              .filetype = 2,
                              .nlink = 1,
                              .atim = state.time,
                              .mtim = state.time,
                              .ctim = state.time}
                     : read_stat(metadata);
  fs->nodes = reserve(fs->nodes, &fs->cap, fs->count + 1, sizeof(Node));
  size_t index = fs->count++;
  fs->nodes[index] = (Node){.name = blob(name),
                            .stat = stat,
                            .parent = parent,
                            .input_parent = parent,
                            .input_index = input_index,
                            .stream = stream};
  if (stat.dev == 0 && stat.filetype == 3) {
    require(fix_is_object(contents) && fix_get_data_type(contents) == FIX_TREE);
    for (uint32_t i = 0; i < fix_len(contents); ++i) {
      size_t child = read_node(entry(contents, i), index, i, false);
      Bytes *name = &fs->nodes[child].name;
      require(name->len && !bytes_equal(name, (const uint8_t *)".", 1) &&
              !bytes_equal(name, (const uint8_t *)"..", 2));
      for (size_t j = 0; j < name->len; ++j)
        require(name->data[j] && name->data[j] != '/');
      for (size_t j = 0; j < fs->nodes[index].children.len; ++j)
        require(!bytes_equal(&fs->nodes[fs->nodes[index].children.data[j]].name,
                             name->data, name->len));
      indices_push(&fs->nodes[index].children, child);
    }
  } else if (stat.dev == 0 && (stat.filetype == 4 || stream)) {
    fs->nodes[index].bytes = blob(contents);
  }
  fs->nodes[index].stat.size = node_size(&fs->nodes[index]);
  return index;
}
static Bytes *byte_list(fix_handle value, size_t *count) {
  require(fix_is_object(value) && fix_get_data_type(value) == FIX_TREE);
  *count = fix_len(value);
  require(*count <= SIZE_MAX / sizeof(Bytes));
  Bytes *list = malloc(*count * sizeof(Bytes));
  require(list || !*count);
  for (size_t i = 0; i < *count; ++i)
    list[i] = blob(entry(value, i));
  return list;
}
static fix_handle original(fix_handle input, size_t index) {
  const Node *node = &state.fs.nodes[index];
  fix_handle parent = node->input_parent == NONE
                          ? entry(input, 3)
                          : entry(original(input, node->input_parent), 2);
  return entry(parent, node->input_index);
}
static fix_handle write_node(fix_handle input, size_t index);
static void write_children(fix_handle input, const Indices *children,
                           size_t index) {
  if (index == children->len) {
    prepare_tree(children->len);
    return;
  }
  fix_handle child = write_node(input, children->data[index]);
  write_children(input, children, index + 1);
  wasm_table_set_0(index, child);
}
static fix_handle write_directory(fix_handle input, const Indices *children) {
  write_children(input, children, 0);
  return make_tree(children->len);
}
static fix_handle write_node(fix_handle input, size_t index) {
  Node *node = &state.fs.nodes[index];
  Stat stat = node->stat;
  stat.size = node_size(node);
  fix_handle name = make_blob(node->name.data, node->name.len);
  fix_handle metadata =
      node->stream ? entry(original(input, index), 1) : write_stat(&stat);
  fix_handle contents =
      !node_file(node)        ? make_blob(node->bytes.data, node->bytes.len)
      : !node_directory(node) ? write_directory(input, &node->children)
                              : entry(original(input, index), 2);
  prepare_tree(3);
  wasm_table_set_0(0, name);
  wasm_table_set_0(1, metadata);
  wasm_table_set_0(2, contents);
  return make_tree(3);
}
__attribute__((export_name("_fix_apply"))) fix_handle apply(fix_handle input) {
  tree(input, 6);
  state = (State){0};
  state.time = integer(entry(input, 5), 8);
  mt19937_seed(&state.random, integer(entry(input, 4), 8));
  state.arguments = byte_list(entry(input, 1), &state.argument_count);
  state.environment = byte_list(entry(input, 2), &state.environment_count);
  fix_handle descriptors = entry(input, 3);
  require(fix_is_object(descriptors) &&
          fix_get_data_type(descriptors) == FIX_TREE);
  for (uint32_t i = 0; i < fix_len(descriptors); ++i)
    indices_push(&state.fs.roots,
                 read_node(entry(descriptors, i), NONE, i, i < 3));
  fs_init(&state.fs);
  guest_run();
  fix_handle roots = write_directory(input, &state.fs.roots);
  prepare_tree(2);
  wasm_table_set_0(0, roots);
  wasm_table_set_0(1, fix_create_blob_i32(state.exit_status));
  fix_handle result = make_tree(2);
  fix_detach_tree(0, 0);
  fix_detach_blob(1, 0);
  fs_destroy(&state.fs);
  for (size_t i = 0; i < state.argument_count; ++i)
    free(state.arguments[i].data);
  for (size_t i = 0; i < state.environment_count; ++i)
    free(state.environment[i].data);
  free(state.arguments);
  free(state.environment);
  return result;
}
