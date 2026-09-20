#ifndef SIMPLE_H
#define SIMPLE_H

#include <stddef.h>

/** A point in 2D space. */
struct point {
    /* X coordinate. */
    int x;
    long long y;
};

typedef unsigned int socklen_t;
struct sockaddr {
    unsigned short sa_family;
    char sa_data[14];
};

/** Add two integers. */
int add(int a, int b);
/* Move a point by (dx, dy). */
int move_point(struct point *p, int dx, int dy);

/** Send bytes on a socket. */
int sock_send(int fd, const void *buf, size_t len, int flags);

/** Connect a socket to an address. */
int sock_connect(int fd, const struct sockaddr *addr, socklen_t addrlen);

/** Set a socket option. */
int sock_setopt(int fd, int level, int name, const void *optval, socklen_t optlen);

/** Address family constant. */
#define TEST_AF_INET 2
/** Function-like macros are not constants. */
#define TEST_MAX(a, b) ((a) > (b) ? (a) : (b))
#define TEST_SHUT_RD 0 /* No more receptions. */

enum {
    /** The first constant. */
    ANON_ONE = 1,
    ANON_TWO = 2,
};

/** An opaque context — incomplete, never defined. */
struct opaque_ctx;
// Ping an opaque context.
int opaque_ping(struct opaque_ctx *ctx);

/** Read a message, returning a borrowed string. */
const char *sock_recv_str(int fd);

/** Byte-order helper. */
unsigned short sock_htons(unsigned short v);

/** A record with bitfields — not representable. */
struct flags {
    unsigned int a : 1;
    unsigned int b : 7;
};
int flags_check(struct flags *f);

#endif
