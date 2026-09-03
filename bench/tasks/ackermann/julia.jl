#!/usr/bin/env julia
# Recursive ackermann — same algorithm as the other language versions.
# Usage: julia julia.jl 2 5
function ack(m, n)
    if m == 0
        return n + 1
    elseif n == 0
        return ack(m - 1, 1)
    else
        return ack(m - 1, ack(m, n - 1))
    end
end

m = parse(Int, ARGS[1])
n = parse(Int, ARGS[2])
println(ack(m, n))
