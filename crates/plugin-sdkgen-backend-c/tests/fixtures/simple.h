#ifndef SIMPLE_H
#define SIMPLE_H

struct point {
    int x;
    long long y;
};

int add(int a, int b);
int move_point(struct point *p, int dx, int dy);

#endif
