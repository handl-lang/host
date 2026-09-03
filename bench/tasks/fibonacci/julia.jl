#!/usr/bin/env julia
# Iterative fibonacci — same algorithm as the other language versions.
# Usage: julia julia.jl 35
function fib(n)
    a, b = 0, 1
    for _ in 1:n
        a, b = b, a + b
    end
    return a
end
n = parse(Int, ARGS[1])
println(fib(n))
