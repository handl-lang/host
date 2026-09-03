// Recursive ackermann — same algorithm as the handl/js/python/rust/c versions.
// Build: javac java.java
// Usage: java java 2 5
public class java {
    static long ack(long m, long n) {
        if (m == 0) return n + 1;
        if (n == 0) return ack(m - 1, 1);
        return ack(m - 1, ack(m, n - 1));
    }

    public static void main(String[] args) {
        long m = Long.parseLong(args[0]);
        long n = Long.parseLong(args[1]);
        System.out.println(ack(m, n));
    }
}
