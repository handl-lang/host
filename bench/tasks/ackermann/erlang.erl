#!/usr/bin/env escript
%% Recursive ackermann — same algorithm as the other language versions.
%% Usage: escript erlang.erl 2 5
main([M, N]) ->
    Mi = list_to_integer(M),
    Ni = list_to_integer(N),
    io:format("~p~n", [ack(Mi, Ni)]).

ack(0, N) -> N + 1;
ack(M, 0) -> ack(M - 1, 1);
ack(M, N) -> ack(M - 1, ack(M, N - 1)).
