#include <fcntl.h>
#include <stdio.h>
#include <sys/mman.h>
#include <unistd.h>

int main(int argc, char **argv) {
  printf("hello, world!\n");
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
