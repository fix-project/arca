#include "module.h"
#include "wasi-api.h"
#include "wasm-rt.h"

#include <arca/sys.h>
#include <assert.h>
#include <stdbool.h>
#include <string.h>

[[noreturn]] void trap(const char *msg) {
  arca_debug_log((const uint8_t *)msg, strlen(msg));
  arca_exit(0);
}

uint64_t check(const char *msg, int64_t ret) {
  if (ret < 0) {
    trap(msg);
  }
  return ret;
}

wasm_rt_memory_t *get_memory(struct w2c_wasi__snapshot__preview1 *module) {
  w2c_module *mod = (w2c_module *)module;
  return w2c_module_memory(mod);
}

/**
 * @brief Gets the arguments.
 *
 * @param argv_ptr Argument array pointer (char**)
 * @param argv_buf_ptr Argument buffer pointer (char*)
 * @return Status code
 */
u32 w2c_wasi__snapshot__preview1_args_get(
    struct w2c_wasi__snapshot__preview1 *module, u32 argv_ptr,
    u32 argv_buf_ptr) {
  return __WASI_ERRNO_FAULT;
}

/**
 * @brief Gets the number of the arguments.
 *
 * @param num_argument_ptr Number of arguments
 * @param size_argument_ptr Size of arguments in bytes
 * @return Status code
 */
u32 w2c_wasi__snapshot__preview1_args_sizes_get(
    struct w2c_wasi__snapshot__preview1 *module, u32 num_argument_ptr,
    u32 size_argument_ptr) {
  return __WASI_ERRNO_FAULT;
}

/**
 * @brief Closes file descriptor.
 *
 * @param fd File descriptor
 * @return Status code
 */
u32 w2c_wasi__snapshot__preview1_fd_close(
    struct w2c_wasi__snapshot__preview1 *module, u32 fd) {
  return __WASI_ERRNO_FAULT;
}

/**
 * @brief Gets file descriptor attributes.
 *
 * @param fd File descriptor
 * @param retptr0 Returns file descriptor attributes as __wasi_fdstat_t
 * @return Status code
 */
u32 w2c_wasi__snapshot__preview1_fd_fdstat_get(
    struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 retptr0) {
  return __WASI_ERRNO_FAULT;
}

/**
 * @brief Seeks to offset in file descriptor.
 *
 * @param fd File descriptor
 * @param offset Number of bytes to seek
 * @param whence Whence to seek from (SEEK_SET, SEEK_CUR, SEEK_END)
 * @param retptr0 Returns resulting offset as int64_t
 * @return Status code
 */
u32 w2c_wasi__snapshot__preview1_fd_seek(
    struct w2c_wasi__snapshot__preview1 *module, u32 fd, u64 offset, u32 whence,
    u32 retptr0) {
  return __WASI_ERRNO_FAULT;
}

/**
 * @brief Writes to file descriptor.
 *
 * @param fd file descriptor
 * @param iovs Array of __wasi_iovec_t structs containing buffers to write from
 * @param iovs_len Number of buffers in iovs
 * @param retptr0 Returns number of bytes written as int32_t
 * @return Status code
 */
u32 w2c_wasi__snapshot__preview1_fd_write(
    struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 iovs, u32 iovs_len,
    u32 retptr0) {
  return __WASI_ERRNO_FAULT;
}

/**
 * @brief Exits process, returning rval to the host environment.
 *
 * @param rval Return value
 */
void w2c_wasi__snapshot__preview1_proc_exit(
    struct w2c_wasi__snapshot__preview1 *module, u32 rvalue) {
  return;
}

int main(int argc, char **argv) {
  w2c_module module;
  wasm2c_module_instantiate(&module, (struct w2c_wasi__snapshot__preview1 *)&module);
  arca_exit(0);
  w2c_module_0x5Fstart(&module);
  // TODO: set the return value correctly
  arca_exit(0);
}
