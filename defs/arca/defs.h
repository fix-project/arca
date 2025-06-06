#pragma once
#include <stddef.h>
#include <stdint.h>

enum arca_syscall {
  // general operational system calls
  SYS_NOP,
  SYS_CLONE,
  SYS_DROP,
  SYS_EXIT,
  SYS_TYPE,

  // object creation
  SYS_CREATE_NULL,
  SYS_CREATE_WORD,
  SYS_CREATE_ATOM,
  SYS_CREATE_ERROR,
  SYS_CREATE_BLOB,
  SYS_CREATE_TREE,
  SYS_CREATE_PAGE,
  SYS_CREATE_TABLE,
  SYS_CREATE_LAMBDA,
  SYS_CREATE_THUNK,

  // object usage
  SYS_READ,
  SYS_WRITE,
  SYS_EQUALS,
  SYS_LENGTH,
  SYS_TAKE,
  SYS_PUT,
  SYS_APPLY,
  SYS_MAP,

  // current arca
  SYS_MMAP,
  SYS_MPROTECT,

  // continuations
  SYS_RETURN_CONTINUATION_LAMBDA,
  SYS_PERFORM_EFFECT,
  SYS_TAILCALL,
  SYS_CAPTURE_CONTINUATION_THUNK,
  SYS_CAPTURE_CONTINUATION_LAMBDA,

  // debug
  SYS_DEBUG_LOG,
  SYS_DEBUG_LOG_INT,
  SYS_DEBUG_SHOW,
  SYS_ERROR_RESET,
  SYS_ERROR_APPEND,
  SYS_ERROR_APPEND_INT,
  SYS_ERROR_RETURN,
};

enum arca_error {
  ERROR_BAD_SYSCALL,
  ERROR_BAD_INDEX,
  ERROR_BAD_TYPE,
  ERROR_BAD_ARGUMENT,
  ERROR_OUT_OF_MEMORY,
  ERROR_INTERRUPTED,
};

enum arca_datatype {
  DATATYPE_NULL,
  DATATYPE_WORD,
  DATATYPE_ATOM,
  DATATYPE_ERROR,
  DATATYPE_BLOB,
  DATATYPE_TREE,
  DATATYPE_PAGE,
  DATATYPE_TABLE,
  DATATYPE_LAMBDA,
  DATATYPE_THUNK,
};

enum arca_entry_mode {
  ENTRY_MODE_NONE,
  ENTRY_MODE_READ_ONLY,
  ENTRY_MODE_READ_WRITE,
};

typedef int64_t arcad;

struct arca_entry {
  enum arca_entry_mode mode;
  enum arca_datatype datatype;
  size_t data;
};
