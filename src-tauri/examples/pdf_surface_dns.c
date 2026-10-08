// For scripts/pdf-surface-check.sh: an LD_PRELOAD shim that appends every
// hostname any process looks up (WebKit's network process included) to the
// file named by $HD_DNS_LOG, one name a line, so the surface check can tell
// whether the preview resolved a name itself rather than through the proxy.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <netdb.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int getaddrinfo(const char *node, const char *service, const struct addrinfo *hints,
                struct addrinfo **res) {
    static int (*real)(const char *, const char *, const struct addrinfo *, struct addrinfo **);
    if (!real) real = dlsym(RTLD_NEXT, "getaddrinfo");
    const char *log = getenv("HD_DNS_LOG");
    if (node && log) {
        int fd = open(log, O_WRONLY | O_APPEND | O_CREAT, 0600);
        if (fd >= 0) {
            dprintf(fd, "%s\n", node);
            close(fd);
        }
    }
    return real(node, service, hints, res);
}
