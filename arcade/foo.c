#include <assert.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/mman.h>
#include <unistd.h>

int main(int argc, char **argv) {
  FILE *ctl, *lctl, *data;
  int id, lid;
  size_t n, count = 0;
  char buf_file[1024] = {0}, buf_listen[1024] = {0}, buf_conn[1024] = {0};

  ctl = fopen("/net/tcp/clone", "w+");
  assert(ctl);
  assert(fprintf(ctl, "announce 0.0.0.0:8080\n") > 0);
  assert(!fflush(ctl));
  fscanf(ctl, "%d", &id);
  assert(snprintf(buf_listen, 1024, "/net/tcp/%d/listen", id) < 1024);

  printf("listening on port 8080\n");
  fflush(stdout);

  for (;;) {
    lctl = fopen(buf_listen, "w+");
    assert(lctl);
    fscanf(lctl, "%d", &lid);
    assert(snprintf(buf_conn, 1024, "/net/tcp/%d/data", lid) < 1024);
    data = fopen(buf_conn, "w+");
    assert(data);
    fgetln(data, &n);
    fprintf(data, "HTTP/1.1 200 OK\r\n"
                  "Content-Type: text/html\r\n"
                  "\r\n"
                  "<h1>Hello, World!</h1>\r\n");
    fprintf(data, "<p>You are visitor #%ld!</p>\n", ++count);
    fflush(data);
    fprintf(lctl, "hangup\n");
    fclose(lctl);
    fclose(data);
  }

  fprintf(ctl, "hangup\n");
  fclose(ctl);
}
