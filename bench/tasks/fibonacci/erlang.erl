#!/usr/bin/env escript
%% Iterative fibonacci — same algorithm as the other language versions.
%% Usage: escript erlang.erl 35
main([N]) ->
    Num = list_to_integer(N),
    {A, _B} = lists:foldl(fun(_, {A, B}) -> {B, A + B} end, {0, 1}, lists:seq(1, Num)),
    io:format("~p~n", [A]).
