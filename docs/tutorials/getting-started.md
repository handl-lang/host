# Getting started with handl

This tutorial walks you through writing and running your first handl expression. By the end you will have `hnl` installed and a working script.

## What you will learn

- How to run the `hnl` command.
- How handl expressions and pipelines look.
- Where to go next for the platform model.

## Prerequisites

You need the handl command-line host. If you have not installed it, follow [How to install handl](../how-to/install-handl) first.

## Your first expression

Create a file named `hello.hnl`:

```handl
greeting: type.string = "Hello, handl!";
greeting
```

Run it:

```sh
hnl eval hello.hnl
```

You should see:

```
"Hello, handl!"
```

`hnl` loaded the default host, which is composed from a kit of plugins. The `eval` capability asked the host to evaluate the file.

## Working with arrays

handl is built around data pipelines. Try grouping a list:

```handl
numbers: (a) -> array.every(a, type.number) = [1, 2, 3, 4, 5];
chunks: type.array = array.chunk(numbers, 2);
chunks
```

This gives:

```
[[1, 2], [3, 4], [5]]
```

## Chaining functions

You can pass the result of one function to another:

```handl
text: type.string = "  hello world  ";
clean: type.string = str.trim(text);
clean
```

Result:

```
"hello world"
```

## Using the crypto module

Some modules talk to platform capabilities. For example, hashing a string:

```handl
hash: type.string = crypto.sha256("hello");
hash
```

Result:

```
"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
```

## Refining types

You can also define your own predicate types. A function that returns a boolean is a valid type:

```handl
Positive = (n: type.number) -> n > 0;
lucky: Positive = 7;
lucky
```

This evaluates to `7`.

## What happens under the hood

`hnl` is not a single, monolithic program. It is an executable `Host` that loads a kit, discovers the capabilities the kit provides, and dispatches your command to the right plugin. You do not need to know this to use handl, but it is the reason the same language can power the CLI, the LSP server, the formatter, and the test runner.

## Next steps

- Build a host from scratch in the [Build a host](./build-a-host) tutorial.
- Read the [language overview](../explanation/language-overview) to learn about syntax and values.
- Browse the [reference](../reference/) for every available function.
