#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>

#include <arca/arca.h>

int main(int argc, char **argv) {
  printf("trying large malloc with default rlimit: ");
  void *p = malloc(512 * 4096);
  printf("%s\n", p ? "succeeded" : "failed");
  free(p);
  arca_setrlimit(1 << 30);
  printf("trying large malloc with raised rlimit: ");
  void *q = malloc(512 * 4096);
  printf("%s\n", q ? "succeeded" : "failed");
  free(q);
}
