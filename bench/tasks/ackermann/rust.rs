// Recursive ackermann — same algorithm as the handl/js/python/c versions.
// Build: rustc -O rust.rs -o rust_ack
// Usage: ./rust_ack 2 5
use std::env;

fn ack(m: u64, n: u64) -> u64 {
    if m == 0 {
        return n + 1;
    }
    if n == 0 {
        return ack(m - 1, 1);
    }
    ack(m - 1, ack(m, n - 1))
}

fn main() {
    let m: u64 = env::args().nth(1).unwrap().parse().unwrap();
    let n: u64 = env::args().nth(2).unwrap().parse().unwrap();
    println!("{}", ack(m, n));
}
