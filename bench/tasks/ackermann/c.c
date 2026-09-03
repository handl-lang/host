/* Recursive ackermann — same algorithm as the handl/js/python/rust versions.
 * Build: gcc -O2 c.c -o c_ack
 * Usage: ./c_ack 2 5
 */
#include <stdio.h>
#include <stdlib.h>

unsigned long long ack(unsigned long long m, unsigned long long n) {
    if (m == 0) return n + 1;
    if (n == 0) return ack(m - 1, 1);
    return ack(m - 1, ack(m, n - 1));
}

int main(int argc, char **argv) {
    unsigned long long m = atoi(argv[1]);
    unsigned long long n = atoi(argv[2]);
    printf("%llu\n", ack(m, n));
    return 0;
}
