#!/usr/bin/env ruby
# Recursive ackermann — same algorithm as the handl/js/python/rust/c versions.
# Usage: ruby ruby.rb 2 5
def ack(m, n)
  if m == 0
    n + 1
  elsif n == 0
    ack(m - 1, 1)
  else
    ack(m - 1, ack(m, n - 1))
  end
end

m = ARGV[0].to_i
n = ARGV[1].to_i
puts ack(m, n)
