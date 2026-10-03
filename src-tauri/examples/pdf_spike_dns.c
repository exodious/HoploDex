// For examples/pdf_spike.sh: an LD_PRELOAD shim that logs every hostname
// any process looks up (WebKit's network process included), so the spike can
// tell whether a preview resolves names itself rather than through the proxy.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <netdb.h>
#include <stdio.h>
#include <unistd.h>

int getaddrinfo(const char *node, const char *service, const struct addrinfo *hints,
                struct addrinfo **res) {
    static int (*real)(const char *, const char *, const struct addrinfo *, struct addrinfo **);
    if (!real) real = dlsym(RTLD_NEXT, "getaddrinfo");
    if (node) dprintf(2, "SPIKE DNS lookup %s (pid %d)\n", node, getpid());
    return real(node, service, hints, res);
}
