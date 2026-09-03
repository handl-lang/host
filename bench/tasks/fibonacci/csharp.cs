// Iterative fibonacci — same algorithm as the other language versions.
// Build: dotnet publish -c Release -o ./cs_fib
// Usage: dotnet cs_fib/fib.dll 35
// Or run directly: dotnet run -- 35
using System;

class Fib {
    static void Main(string[] args) {
        long n = long.Parse(args[0]);
        long a = 0, b = 1;
        for (long i = 0; i < n; i++) {
            long tmp = a + b;
            a = b;
            b = tmp;
        }
        Console.WriteLine(a);
    }
}
