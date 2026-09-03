/* Iterative fibonacci — same algorithm as the handl/js/python/rust versions.
 * Build: gcc -O2 c.c -o c_fib
 * Usage: ./c_fib 35
 */
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char **argv) {
    int n = atoi(argv[1]);
    unsigned long long a = 0, b = 1;
    for (int i = 0; i < n; i++) {
        unsigned long long tmp = a + b;
        a = b;
        b = tmp;
    }
    printf("%llu\n", a);
    return 0;
}
