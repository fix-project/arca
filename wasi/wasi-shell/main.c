#include "module.h"
#include "wasi-api.h"
#include "wasm-rt.h"

#include <arca/sys.h>
#include <assert.h>
#include <stdbool.h>
#include <string.h>
#include <stdio.h>
#include <unistd.h>
#include <sys/uio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <stdlib.h>
#include <fcntl.h>
#include <time.h>


arcad current_arg = 0;

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
  arca_debug_log("args", 4);
  u64 base = (u64) (((w2c_module*) (module))->w2c_memory.data);
  uint8_t* buf_addr = (((w2c_module*) (module))->w2c_memory.data + argv_buf_ptr);
  u32* ptr_array_addr = (u32*) (((w2c_module*) (module))->w2c_memory.data + argv_ptr);
  u64 num_arg_64;
  arca_length(current_arg, &num_arg_64);
  for (u64 i = 0; i < num_arg_64; i++) {
    arcad blob_desc = arca_tuple_get(current_arg, i);
    u64 blob_len;
    arca_length(blob_desc, &blob_len);
    arca_blob_read(blob_desc, 0, buf_addr, blob_len);
    arca_debug_log(buf_addr, blob_len);
    *ptr_array_addr = (u32) (buf_addr - base);
    buf_addr += blob_len;
    ptr_array_addr += 1;
  }
  return __WASI_ERRNO_SUCCESS;
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
  arca_debug_log("sizes", 5);
  current_arg = arca_argument();
  u64 num_arg_64;
  arca_length(current_arg, &num_arg_64);
  //num_arg_64--;
  u32*  num_arg_32 = (u32*) (((w2c_module*) (module))->w2c_memory.data + num_argument_ptr);
  (*num_arg_32) = num_arg_64;
  u32 size = 0;
  for (u32 i = 0; i < *num_arg_32; i++) {
    arcad arg = arca_tuple_get(current_arg, i);
    u64 cur_size;
    arca_length(arg, &cur_size);
    size += cur_size;
  }
  *((u32*) (((w2c_module*) (module))-> w2c_memory.data + size_argument_ptr)) = size;
  arca_debug_log_int("size", 4, size);
  return __WASI_ERRNO_SUCCESS;
}

/**
 * @brief Closes file descriptor.
 *
 * @param fd File descriptor
 * @return Status code
 */
u32 w2c_wasi__snapshot__preview1_fd_close(
    struct w2c_wasi__snapshot__preview1 *module, u32 fd) {
  arca_debug_log("close", 5);
  int result = close(fd);
  if (result == 0) {
    return __WASI_ERRNO_SUCCESS;
  } else {
    return __WASI_ERRNO_FAULT;
  }
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
  //struct stat stats;
  //fstat(fd, &stats);
  arca_debug_log("stat", 4);
  arca_debug_log_int("fd", 2, fd);
  struct __wasi_fdstat_t stat;

  if (fd == 0) {
    stat.fs_filetype = __WASI_FILETYPE_CHARACTER_DEVICE;
    stat.fs_rights_base = __WASI_RIGHTS_FD_READ;
    stat.fs_rights_inheriting = __WASI_RIGHTS_FD_READ;
    stat.fs_flags = 0;
  } else if (fd == 1) {
    stat.fs_filetype = __WASI_FILETYPE_CHARACTER_DEVICE;
    stat.fs_rights_base = __WASI_RIGHTS_FD_WRITE;
    stat.fs_rights_inheriting = __WASI_RIGHTS_FD_WRITE;
    stat.fs_flags = 0;
  } else if (fd == 2) {
    stat.fs_filetype = __WASI_FILETYPE_CHARACTER_DEVICE;
    stat.fs_rights_base = __WASI_RIGHTS_FD_WRITE;
    stat.fs_rights_inheriting = __WASI_RIGHTS_FD_WRITE;
    stat.fs_flags = 0;
  } else if (fd == 3) {
    stat.fs_filetype = __WASI_FILETYPE_DIRECTORY;
    stat.fs_rights_base = __WASI_RIGHTS_FD_READ | __WASI_RIGHTS_FD_WRITE | __WASI_RIGHTS_FD_READDIR;
    stat.fs_rights_inheriting = __WASI_RIGHTS_FD_READ | __WASI_RIGHTS_FD_WRITE | __WASI_RIGHTS_FD_READDIR;
    stat.fs_flags = 0;
  } else {
    return __WASI_ERRNO_FAULT;
  }
  struct __wasi_fdstat_t* retptr = (struct __wasi_fdstat_t *) (((w2c_module*) (module))->w2c_memory.data + retptr0);
  (*retptr) = stat;
  return __WASI_ERRNO_SUCCESS;
}

u32 w2c_wasi__snapshot__preview1_fd_fdstat_set_flags(struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 fdflags){
  arca_debug_log("flag", 4);
  return __WASI_ERRNO_FAULT;
}

u32 w2c_wasi__snapshot__preview1_fd_prestat_get(struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 retptr0 ) {
  arca_debug_log("prestat", 7);
  char fd_buf[9];
  snprintf(fd_buf, sizeof(fd_buf), "fd %u", fd);
  //arca_debug_log(fd_buf, 9);
  if (fd > 3) {
    return __WASI_ERRNO_BADF;
  }
  struct __wasi_prestat_t stat;
  stat.tag = __WASI_PREOPENTYPE_DIR;
  if (fd == 0) {
    //stdin
    stat.u.dir.pr_name_len = 5;
  } else if (fd == 1 || fd == 2) {
    //stdout or stderr
    stat.u.dir.pr_name_len = 6;
  } else {
    //.
    stat.u.dir.pr_name_len = 1;
  }
  struct __wasi_prestat_t* retptr = (struct __wasi_prestat_t*) (((w2c_module*) (module))->w2c_memory.data + retptr0);
  (*retptr) = stat;
  return __WASI_ERRNO_SUCCESS;
}

u32 w2c_wasi__snapshot__preview1_fd_prestat_dir_name(struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 path, u32 path_len ) {
  arca_debug_log("name", 4);
  if (fd == 3) {
    strcpy((((w2c_module*) (module))->w2c_memory.data + path), ".");
  }
  return __WASI_ERRNO_SUCCESS;
}

u32 w2c_wasi__snapshot__preview1_path_open( struct w2c_wasi__snapshot__preview1 *module, u32 fd,
                   u32 dirflags,
                   u32 path,
                   u32 path_len,
                   u32 oflags,
                   u64 fs_rights_base,
                   u64 fs_rights_inheriting,
                   u32 fdflags,
                   u32 retptr0) {
  arca_debug_log("open", 4);
  char *path_buf = (char *) (((w2c_module*) (module))->w2c_memory.data + path);
  char *base_buf = (char *) (((w2c_module*) (module))->w2c_memory.data + fd);
  arca_debug_log(path_buf, 10);
  arca_debug_log(base_buf, 5);
  u32 *retptr = (u32 *) (((w2c_module*) (module))->w2c_memory.data + retptr0);
  u32 filed = open(path_buf, oflags);
  *retptr = filed;
  arca_debug_log_int("fd", 2, filed);
  return __WASI_ERRNO_SUCCESS;
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
  arca_debug_log("seek", 4);
  arca_debug_log_int("fd", 2, fd);
  u32 result = lseek(fd, offset, whence);
  if (result == offset - 1) {
    return __WASI_ERRNO_FAULT;
  } else {
    retptr0 = result;
    return __WASI_ERRNO_SUCCESS;
  }
}

/**
 * @brief Reads from file descriptor.
 *
 * @param fd File descriptor
 * @param iovs Array of __wasi_iovec_t structs containing buffers to read into
 * @param iovs_len Number of buffers in iovs
 * @param retptr0 Returns number of bytes read as u32
 * @return u32 Status code
 */
u32 w2c_wasi__snapshot__preview1_fd_read(struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 iovs, u32 iovs_len,
    u32 retptr0 )
{
  // arca_debug_log("read", 4);
  // arca_debug_log_int("iovs len", 8, iovs_len);
  // arca_debug_log_int("fd", 2, fd);
  // u64 num_buffers = iovs_len;
  // int32_t bytes_read = 0;
  // __wasi_iovec_t* buf_array = (__wasi_iovec_t*) (((w2c_module*) (module))->w2c_memory.data + iovs);
  // while (num_buffers > 0) {
  //   u32 buf_ptr = buf_array -> buf;
  //   u64 len = (u64) buf_array -> buf_len;
  //   arca_debug_log_int("len", 3, len);
  //   uint8_t* addr = (uint8_t*) (((w2c_module*) (module))->w2c_memory.data + buf_ptr);
  //   size_t total_bytes_read = 0;
  //   while (total_bytes_read < len) {
  //     size_t bytes_read_inner = read(fd, addr + total_bytes_read, len - total_bytes_read);
  //     assert(bytes_read_inner >= 0);
  //     if (bytes_read_inner == 0) {
  //       break;
  //     } else {
  //       total_bytes_read += bytes_read_inner;
  //     }
  //   }
  //   bytes_read += total_bytes_read;
  //   if (total_bytes_read < len) {
  //     break;
  //   }
  //   //arca_debug_log(addr, 10);
  //   buf_array += 1;
  //   num_buffers--;
  // }
  // arca_debug_log_int("bytes", 5, bytes_read);
  // int32_t* retptr = (int32_t*) (((w2c_module*) (module))->w2c_memory.data + retptr0);
  // *retptr = bytes_read;
  // return __WASI_ERRNO_SUCCESS;  
  arca_debug_log("read", 4);
  u64 num_buffers = iovs_len;
  __wasi_iovec_t* buf_array = (__wasi_iovec_t*) (((w2c_module*) (module))->w2c_memory.data + iovs);
  u64 total_bytes_read = 0;
  while (num_buffers > 0) {
    u32 buffer_ptr = buf_array -> buf;
    u32 buffer_len = buf_array -> buf_len;
    uint8_t* addr = (uint8_t*) (((w2c_module*) (module))->w2c_memory.data + buffer_ptr);
    size_t bytes_read = read(fd, addr, (u64) buffer_len);
    buf_array += 1;
    num_buffers--;
    total_bytes_read += bytes_read;
  }
  int32_t* retptr = (int32_t*) (((w2c_module*) (module))->w2c_memory.data + retptr0);
  *retptr = total_bytes_read;
  return __WASI_ERRNO_SUCCESS;
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
  arca_debug_log("write", 5);
  arca_debug_log_int("fd", 2, fd);
  // char fd_buf[9];
  // snprintf(fd_buf, sizeof(fd_buf), "fd %u", fd);
  // arca_debug_log(fd_buf, 9);
  char len_buf[9];
  snprintf(len_buf, sizeof(len_buf), "len %u", iovs_len);
  //arca_debug_log(len_buf, 9);
  u64 num_buffers = iovs_len;
  int32_t bytes_written = 0;
  __wasi_iovec_t* buf_array = (__wasi_iovec_t*) (((w2c_module*) (module))->w2c_memory.data + iovs);
  while (num_buffers > 0) {
    char num[13];
    snprintf(num, sizeof(num), "num %u", num_buffers);
    //arca_debug_log(num, 13);
    u32 buf_ptr = buf_array -> buf;
    u64 len = buf_array -> buf_len;
    uint8_t* addr = (uint8_t*) (((w2c_module*) (module))->w2c_memory.data + buf_ptr);
    //arca_debug_log(addr, len);
    write(fd, addr, len);
    //buf_array += sizeof(__wasi_iovec_t);
    buf_array += 1;
    num_buffers--;
    bytes_written += len;
  }
  int32_t* retptr = (int32_t*) (((w2c_module*) (module))->w2c_memory.data + retptr0);
  *retptr = bytes_written;
  //arca_exit(arca_blob_create("end of write", 12));
  //arca_debug_log("done write", 10);
  return __WASI_ERRNO_SUCCESS;
}

/**
 * @brief Exits process, returning rval to the host environment.
 *
 * @param rval Return value
 */
void w2c_wasi__snapshot__preview1_proc_exit(
    struct w2c_wasi__snapshot__preview1 *module, u32 rvalue) {
  arca_debug_log("exit", 4);
  exit(rvalue);
}

u32 w2c_wasi__snapshot__preview1_poll_oneoff( struct w2c_wasi__snapshot__preview1 *module, u32 in, u32 out, u32 nsubscriptions, u32 retptr0 )
{
  arca_debug_log("poll", 4);
  return 0;
}

u32 w2c_wasi__snapshot__preview1_fd_filestat_get(struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 retptr0) {
  arca_debug_log("filestat get", 12);
  struct __wasi_filestat_t stat;
  stat.dev = 1;
  stat.nlink = 1;
  stat.size = 0;//?
  stat.atim = 1757913961613440734;
  stat.mtim = 1757913961613440734;
  stat.ctim = 1757913961613440734;
  stat.filetype = __WASI_FILETYPE_REGULAR_FILE;
  struct __wasi_filestat_t* retptr = (struct __wasi_filestat_t *) (((w2c_module*) (module))->w2c_memory.data + retptr0);
  (*retptr) = stat;
  return __WASI_ERRNO_SUCCESS;
}

u32 w2c_wasi__snapshot__preview1_path_unlink_file( struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 path, u32 path_len )
{
  arca_debug_log("unlink file", 11);
  return 0;
}

u32 w2c_wasi__snapshot__preview1_path_remove_directory( struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 path, u32 path_len )
{
  arca_debug_log("remove dir", 10);
  return 0;
}

u32 w2c_wasi__snapshot__preview1_path_create_directory( struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 path, u32 path_len )
{
  arca_debug_log("create dir", 10);
  return 0;
}

u32 w2c_wasi__snapshot__preview1_path_rename( struct w2c_wasi__snapshot__preview1 *module, u32 fd,
                     u32 old_path,
                     u32 old_path_len,
                     u32 new_fd,
                     u32 new_path,
                     u32 new_path_len )
{
  arca_debug_log("path rename", 11);
  return 0;
}

u32 w2c_wasi__snapshot__preview1_clock_time_get( struct w2c_wasi__snapshot__preview1 *module, u32 id, u64 precision, u32 retptr0 )
{
  arca_debug_log("clock", 5);
  u64* retptr = (u64 *) (((w2c_module*) (module))->w2c_memory.data + retptr0);
  (*retptr) = 0;
  return __WASI_ERRNO_SUCCESS;
}

u32 w2c_wasi__snapshot__preview1_fd_readdir( struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 buf, u32 buf_len, u64 cookie, u32 retptr0 )
{
  arca_debug_log("readdir", 7);
  return __WASI_ERRNO_FAULT;
}

u32 w2c_wasi__snapshot__preview1_path_filestat_get( struct w2c_wasi__snapshot__preview1 *module, u32 fd, u32 flags, u32 path, u32 path_len, u32 retptr0 )
{
  arca_debug_log("file get", 8);
  return __WASI_ERRNO_FAULT;
}

u32 w2c_wasi__snapshot__preview1_environ_sizes_get( struct w2c_wasi__snapshot__preview1 *module, u32 retptr0, u32 retptr1 )
{
  arca_debug_log("envi size", 9);
  u32* retptr = (u32 *) (((w2c_module*) (module))->w2c_memory.data + retptr0);
  (*retptr) = 0;
  u32* ptr1 = (u32 *) (((w2c_module*) (module))->w2c_memory.data + retptr1);
  (*ptr1) = 0;
  return __WASI_ERRNO_SUCCESS;
}

u32 w2c_wasi__snapshot__preview1_environ_get( struct w2c_wasi__snapshot__preview1 *module, u32 environ, u32 environ_buf )
{
  arca_debug_log("envi get", 8);
  return __WASI_ERRNO_FAULT;
}


int main(int argc, char **argv) {
  w2c_module module;
  wasm2c_module_instantiate(&module, (struct w2c_wasi__snapshot__preview1 *)&module);
  w2c_module_0x5Fstart(&module);
  // TODO: set the return value correctly;
  w2c_wasi__snapshot__preview1_proc_exit((struct w2c_wasi__snapshot__preview1 *)&module, 0);
  arca_exit(arca_blob_create("hi", 2));
}
