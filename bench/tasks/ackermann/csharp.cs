// Recursive ackermann — same algorithm as the other language versions.
// Build: dotnet publish -c Release -o ./cs_ack
// Usage: dotnet cs_ack/ack.dll 2 5
using System;

class Ack {
    static long AckFn(long m, long n) {
        if (m == 0) return n + 1;
        if (n == 0) return AckFn(m - 1, 1);
        return AckFn(m - 1, AckFn(m, n - 1));
    }

    static void Main(string[] args) {
        long m = long.Parse(args[0]);
        long n = long.Parse(args[1]);
        Console.WriteLine(AckFn(m, n));
    }
}
