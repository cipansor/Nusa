#include <stdio.h>
#include <stdbool.h>
static bool prima(long n) {
    if (n < 2) return false;
    for (long i = 2; i * i <= n; i++) {
        if (n % i == 0) return false;
    }
    return true;
}
int main(void) {
    long c = 0;
    for (long k = 2; k < 200000; k++) if (prima(k)) c++;
    printf("%ld\n", c);
    return 0;
}
