#ifndef SIMPLE_H
#define SIMPLE_H

/** A point in 2D space. */
struct point {
    int x;
    long long y;
};

/** Add two integers. */
int add(int a, int b);
int move_point(struct point *p, int dx, int dy);

#endif
