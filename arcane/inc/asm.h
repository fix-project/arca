#ifndef _ARCA_ASM_H
#define _ARCA_ASM_H

// general runtime calls
#define __NR_nop 0
#define __NR_clone 1
#define __NR_drop 2
#define __NR_exit 3
#define __NR_type 4
#define __NR_get_argument 5

// constructors
#define __NR_create_null 6
#define __NR_create_word 7
#define __NR_create_blob 8
#define __NR_create_tree 9
#define __NR_create_page 10
#define __NR_create_table 11
#define __NR_create_function 12

// consumers
#define __NR_read 13
#define __NR_write 14
#define __NR_equals 15
#define __NR_length 16
#define __NR_take 17
#define __NR_put 18
#define __NR_get 19
#define __NR_set 20
#define __NR_apply 21
#define __NR_map 22

// address space
#define __NR_mmap 23
#define __NR_mprotect 24
#define __NR_compat_mmap 30

// control flow
#define __NR_call_with_current_continuation 25
#define __NR_get_continuation 29
#define __NR_force 30

// debug
#define __NR_debug_log 26
#define __NR_debug_log_int 27
#define __NR_debug_show 28

#define __ERR_bad_syscall 0
#define __ERR_bad_index 1
#define __ERR_bad_type 2
#define __ERR_bad_argument 3
#define __ERR_out_of_memory 4
#define __ERR_interrupted 5

#define __TYPE_null 0
#define __TYPE_word 1
#define __TYPE_blob 2
#define __TYPE_tuple 3
#define __TYPE_page 4
#define __TYPE_table 5
#define __TYPE_function 6

#define __MODE_none 0
#define __MODE_read_only 1
#define __MODE_read_write 2

#endif
