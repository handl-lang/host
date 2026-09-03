// Iterative fibonacci — same algorithm as the handl/js/python/rust/c versions.
// Build: javac java.java
// Usage: java Fib 35
public class java {
    public static void main(String[] args) {
        long n = Long.parseLong(args[0]);
        long a = 0, b = 1;
        for (long i = 0; i < n; i++) {
            long tmp = a + b;
            a = b;
            b = tmp;
        }
        System.out.println(a);
    }
}
