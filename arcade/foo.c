#include <fcntl.h>
#include <stdio.h>
#include <sys/mman.h>
#include <unistd.h>

// volatile char buf[1024];
// volatile char buf[4 * 1024];
// volatile char buf[8 * 1024];
// volatile char buf[16 * 1024];
// volatile char buf[32 * 1024];
// volatile char buf[64 * 1024];
// volatile char buf[128 * 1024];
// volatile char buf[256 * 1024];
// volatile char buf[512 * 1024];
// volatile char buf[1024 * 1024];
volatile char buf[2 * 1024 * 1024];
// volatile char buf[4 * 1024 * 1024];
// volatile char buf[8 * 1024 * 1024];

int main(int argc, char **argv) {
  printf("hello, world!\n");
  char x = buf[sizeof(buf) - 1];
  FILE *f = fopen("/mnt/output.txt", "w");
  int result = fprintf(f, "hello from Arca\n");
  fclose(f);
  FILE *g = fopen("/mnt/input.txt", "r");
  char buf[1024];
  fgets(buf, sizeof(buf), g);
  fclose(g);
  printf("input said: %s\n", buf);
  fflush(stdout);
}
