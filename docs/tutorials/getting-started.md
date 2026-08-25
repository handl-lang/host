# Getting started with handl

This tutorial walks you through writing and running your first handl expression.

## What is handl?

handl is a small, functional language for transforming data. Values are JSON-compatible — strings, numbers, booleans, arrays, and objects — and you call functions in modules using the form `module.function(args)`.

## Prerequisites

- A working handl runtime (see [How to install hnl](../how-to/install-handl)).
- A text editor.

## Your first expression

Create a file named `hello.hnl`:

```handl
"Hello, handl!"
```

Run it:

```sh
hnl run hello.hnl
```

You should see:

```
"Hello, handl!"
```

## Working with arrays

handl is designed around data pipelines. Try a few utility functions:

```handl
numbers = [1, 2, 3, 4, 5]
array.chunk(numbers, 2)
```

This groups the list into pairs:

```
[[1, 2], [3, 4], [5]]
```

## Chaining functions

You can pass the result of one function to another:

```handl
text = "  hello world  "
str.trim(text)
```

Result:

```
"hello world"
```

## Using the crypto module

Some modules communicate with platform capabilities. For example, hashing a string:

```handl
crypto.sha256("hello")
```

Result:

```
"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
```

## Next steps

- Read the [language overview](../explanation/language-overview) to learn about syntax and values.
- Browse the [reference](../reference/) for every available function.
- Try the [How-to guides](../how-to/) for focused recipes.
