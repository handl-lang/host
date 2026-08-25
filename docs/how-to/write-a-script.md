# Write a handl script

This guide shows how to write, save, and run a self-contained handl script.

## File layout

A handl script is a text file with the `.hnl` extension. It contains an expression that the runtime evaluates and prints.

Example `greet.hnl`:

```handl
name = "handl"
str.concat(["Hello, ", name, "!"])
```

## Running the script

Use the CLI:

```sh
hnl run greet.hnl
```

## Using modules

Import capabilities from modules as needed. Most modules are available by default in the runtime; you call them with `module.function` syntax.

```handl
raw = "  padded  "
clean = str.trim(raw)
upper = str.upper(clean)
upper
```

This evaluates to `"PADDED"`.

## Working with JSON data

handl is JSON-native. You can read and transform JSON values directly:

```handl
data = {
  "name": "handl",
  "version": "0.1.0"
}
data.name
```

Result:

```
"handl"
```

## Tips

- Keep one expression per script or clearly chain let bindings.
- Refer to the [reference](../reference/lib-utils) for `array`, `str`, and `math` helpers.
- Use `math.sum` and `math.avg` for numeric summaries, `array.compact` to remove nulls.
