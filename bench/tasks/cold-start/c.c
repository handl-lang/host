/* Cold-start benchmark: print and exit.
 * Build: gcc -O2 c.c -o c_cold
 * Usage: ./c_cold
 */
#include <stdio.h>

int main(void) {
    puts("Hello, world!");
    return 0;
}
