#!/usr/bin/env ruby
# Iterative fibonacci — same algorithm as the handl/js/python/rust/c versions.
# Usage: ruby ruby.rb 35
n = ARGV[0].to_i
a, b = 0, 1
n.times { a, b = b, a + b }
puts a
