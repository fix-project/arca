#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <unistd.h>

int main(int argc, char **argv) {
  printf("hello, world!\n");
  FILE *f = fopen("output.txt", "w");
  fprintf(f, "hello from userspace");
  fclose(f);
  char buf[1024];
  printf("enter some text: ");
  fflush(stdout);
  fgets(buf, sizeof(buf), stdin);
  printf("kernel said: %s\n", buf);
  exit(0);
}
