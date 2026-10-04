typedef unsigned int u32;
typedef unsigned long long u64;
typedef long long i64;
#define IMPORT(name) __attribute__((import_module("wasi_snapshot_preview1"), import_name(#name)))
IMPORT(args_sizes_get) u32 args_sizes_get(u32 *, u32 *);
IMPORT(args_get) u32 args_get(char **, char *);
IMPORT(environ_sizes_get) u32 environ_sizes_get(u32 *, u32 *);
IMPORT(environ_get) u32 environ_get(char **, char *);
IMPORT(clock_time_get) u32 clock_time_get(u32, u64, u64 *);
IMPORT(random_get) u32 random_get(void *, u32);
IMPORT(fd_prestat_get) u32 fd_prestat_get(u32, void *);
IMPORT(fd_prestat_dir_name) u32 fd_prestat_dir_name(u32, void *, u32);
IMPORT(fd_read) u32 fd_read(u32, const void *, u32, u32 *);
IMPORT(fd_write) u32 fd_write(u32, const void *, u32, u32 *);
IMPORT(fd_pread) u32 fd_pread(u32, const void *, u32, u64, u32 *);
IMPORT(fd_pwrite) u32 fd_pwrite(u32, const void *, u32, u64, u32 *);
IMPORT(fd_renumber) u32 fd_renumber(u32, u32);
IMPORT(fd_seek) u32 fd_seek(u32, i64, u32, u64 *);
IMPORT(fd_filestat_get) u32 fd_filestat_get(u32, void *);
IMPORT(fd_fdstat_get) u32 fd_fdstat_get(u32, void *);
IMPORT(path_open) u32 path_open(u32, u32, const char *, u32, u32, u64, u64, u32, u32 *);
IMPORT(path_create_directory) u32 path_create_directory(u32, const char *, u32);
IMPORT(path_rename) u32 path_rename(u32, const char *, u32, u32, const char *, u32);
IMPORT(fd_readdir) u32 fd_readdir(u32, void *, u32, u64, u32 *);
IMPORT(fd_close) u32 fd_close(u32);
IMPORT(proc_exit) __attribute__((noreturn)) void proc_exit(u32);

struct iovec { void *data; u32 length; };

static void check(int condition, u32 status) {
    if (!condition) proc_exit(status);
}

static int equal(const unsigned char *left, const unsigned char *right, u32 length) {
    for (u32 i = 0; i < length; i++) if (left[i] != right[i]) return 0;
    return 1;
}

static void nested_exit(void) { proc_exit(7); }

void _start(void) {
    u32 count, length, used;
    u64 first, second, position;
    unsigned char bytes[128] __attribute__((aligned(8)));
    check(args_sizes_get(&count, &length) == 0 && count == 2 && length == 12, 101);
    check(clock_time_get(1, 0, &first) == 0 && first == 102, 102);
    check(clock_time_get(0, 0, &second) == 0 && second == first + 1, 103);
    char *arguments[2];
    char strings[64];
    check(args_get(arguments, strings) == 0 && equal((void *)arguments[0], (void *)"fixture", 8) && equal((void *)arguments[1], (void *)"foo", 4), 104);
    check(environ_sizes_get(&count, &length) == 0 && count == 1 && length == 12, 105);
    char *environment[1];
    check(environ_get(environment, strings) == 0 && equal((void *)environment[0], (void *)"HELLO=world", 12), 106);
    const unsigned char random_prefix[16] = {0x3e,0xdc,0x41,0xcb,0xc5,0x37,0xe8,0x28,0x8b,0xf9,0x40,0x3e,0x7c,0x3a,0xfd,0xfd};
    check(random_get(bytes, 5) == 0 && random_get(bytes + 5, 11) == 0 && equal(bytes, random_prefix, 16), 107);
    check(fd_prestat_get(3, bytes) == 0 && *(u32 *)(bytes + 4) == 5, 108);
    check(fd_prestat_dir_name(3, bytes, 5) == 0 && equal(bytes, (void *)"/work", 5), 109);
    struct iovec input = {bytes, 128};
    check(fd_read(0, &input, 1, &used) == 0 && used == 5 && equal(bytes, (void *)"input", 5), 110);
    check(fd_read(0, &input, 1, &used) == 0 && used == 0, 111);
    check(path_create_directory(3, "nested", 6) == 0, 112);
    u32 file;
    u64 rights = (1ull << 28) - 1;
    check(path_open(3, 0, "nested/file", 11, 1, rights, rights, 0, &file) == 0, 113);
    check(fd_fdstat_get(file, bytes) == 0 && (*(u64 *)(bytes + 8) & (1ull << 9)) == 0 && *(u64 *)(bytes + 16) == 0, 125);
    struct iovec vectors[2] = {{"hello", 5}, {" world", 6}};
    check(fd_write(file, vectors, 2, &used) == 0 && used == 11, 114);
    check(fd_seek(file, 0, 0, &position) == 0 && position == 0, 115);
    check(fd_read(file, &input, 1, &used) == 0 && used == 11 && equal(bytes, (void *)"hello world", 11), 116);
    check(fd_filestat_get(file, bytes) == 0 && *(u64 *)(bytes + 32) == 11, 117);
    check(path_rename(3, "nested/file", 11, 3, "result", 6) == 0, 118);
    check(fd_readdir(3, bytes, 128, 0, &used) == 0 && used > 48, 119);
    u32 reader, writer, unused;
    check(path_open(3, 0, "result", 6, 0, 1ull << 1, 0, 0, &reader) == 0, 126);
    check(path_open(3, 0, "result", 6, 0, 1ull << 6, 0, 0, &writer) == 0, 127);
    check(fd_pread(reader, &input, 1, 0, &used) == 76, 128);
    check(fd_pwrite(writer, vectors, 2, 0, &used) == 76, 129);
    check(fd_pread(file, &input, 1, 0, &used) == 0 && used == 11 && equal(bytes, (void *)"hello world", 11), 130);
    struct iovec overlapping[2] = {{0, 5}, {bytes, 6}};
    overlapping[0].data = &overlapping[1];
    check(fd_pread(file, overlapping, 2, 0, &used) == 0 && used == 11 &&
          equal((void *)&overlapping[1], (void *)"hello", 5) &&
          equal(bytes, (void *)" world", 6), 141);
    struct iovec invalid[2] = {{"X", 1}, {(void *)0xffffffffu, 1}};
    check(fd_pwrite(file, invalid, 2, 0, &used) == 21, 142);
    check(fd_pread(file, &input, 1, 0, &used) == 0 && used == 11 &&
          equal(bytes, (void *)"hello world", 11), 143);
    check(path_open(3, 0, "missing/", 8, 1, rights, 0, 0, &unused) == 54, 131);
    check(path_open(3, 0, "missing", 7, 0, rights, 0, 0, &unused) == 44, 132);
    check(path_rename(3, "result", 6, 3, "missing/", 8) == 54, 133);
    check(path_rename(3, "result", 6, 3, "result/", 7) == 54, 134);
    check(fd_close(reader) == 0 && fd_close(writer) == 0, 135);
    check(fd_renumber(file, reader) == 0, 136);
    check(fd_close(file) == 8, 137);
    check(fd_renumber(reader, 100) == 0, 138);
    check(fd_pread(100, &input, 1, 0, &used) == 0 && used == 11 && equal(bytes, (void *)"hello world", 11), 139);
    check(fd_close(99) == 8 && fd_close(100) == 0, 140);
    check(path_open(3, 0, "../escape", 9, 1, rights, 0, 0, &file) == 76, 121);
    struct iovec output = {"flatware-ok", 11};
    check(fd_write(1, &output, 1, &used) == 0 && used == 11, 122);
    struct iovec error = {"stderr-ok", 9};
    check(fd_write(2, &error, 1, &used) == 0 && used == 9, 123);
    nested_exit();
    output.data = "after-exit"; output.length = 10;
    fd_write(1, &output, 1, &used);
}
