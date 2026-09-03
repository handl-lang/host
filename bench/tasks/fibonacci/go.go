// Iterative fibonacci — same algorithm as the C/Rust/JS versions.
// Build: go build -o go_fib go.go
// Usage: ./go_fib 35
package main

import (
	"fmt"
	"os"
	"strconv"
)

func main() {
	n, _ := strconv.Atoi(os.Args[1])
	a, b := uint64(0), uint64(1)
	for i := 0; i < n; i++ {
		a, b = b, a+b
	}
	fmt.Println(a)
}
