// Recursive ackermann — same algorithm as the C/Rust/JS versions.
// Build: go build -o go_ack go.go
// Usage: ./go_ack 2 5
package main

import (
	"fmt"
	"os"
	"strconv"
)

func ack(m, n uint64) uint64 {
	if m == 0 {
		return n + 1
	}
	if n == 0 {
		return ack(m-1, 1)
	}
	return ack(m-1, ack(m, n-1))
}

func main() {
	m, _ := strconv.ParseUint(os.Args[1], 10, 64)
	n, _ := strconv.ParseUint(os.Args[2], 10, 64)
	fmt.Println(ack(m, n))
}
