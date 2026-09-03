// Cold-start benchmark: print and exit.
// Build: dotnet publish -c Release -o ./cs_cold
// Usage: dotnet cs_cold/cold.dll
using System;

class Cold {
    static void Main(string[] args) {
        Console.WriteLine("Hello, world!");
    }
}
