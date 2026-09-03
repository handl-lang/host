// Iterative fibonacci — same algorithm as the handl/js/python/c versions.
// Build: rustc -O rust.rs -o rust_fib
// Usage: ./rust_fib 35
use std::env;

fn main() {
    let n: u64 = env::args().nth(1).unwrap().parse().unwrap();
    let mut a: u64 = 0;
    let mut b: u64 = 1;
    for _ in 0..n {
        let tmp = a + b;
        a = b;
        b = tmp;
    }
    println!("{a}");
}
