#pragma push_macro("glibc_clang_prereq")
#define __glibc_clang_prereq(maj, min) 0
#include_next <math.h>
#pragma pop_macro("glibc_clang_prereq")
