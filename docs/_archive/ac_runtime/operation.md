**ac_runtime > operation**

# Module: operation

## Contents

**Structs**

- [`OperationSig`](#operationsig) - The signature of an operation — its name and parameter types.

**Traits**

- [`Operation`](#operation) - A typed operation that signals instead of completing.

---

## ac_runtime::operation::Operation

*Trait*

A typed operation that signals instead of completing.

When the VM dispatches a call and the name resolves to an operation
(via [`Host::operation`](crate::Host::operation)), the operation's
[`perform`](Operation::perform) method is called. If it returns
`Err(Error::Operation { ... })`, the VM captures the continuation
and yields. If it returns `Ok(value)`, the operation completed
without signaling.

# Implementing

```ignore
struct Fetch;

impl Operation<MyValue> for Fetch {
    fn signature(&self) -> OperationSig<MyValue> {
        OperationSig {
            name: "fetch".to_string(),
            params: vec![],
            return_schema: None,
        }
    }

    fn perform(&self, args: &[MyValue], _ctx: &mut dyn Context<MyValue>) -> Result<MyValue, Error<MyValue>> {
        // Signal: yield to the caller with operation name + args
        Err(Error::Operation {
            name: "fetch".to_string(),
            args: args.to_vec(),
        })
    }
}
```

**Methods:**

- `signature`: Returns the signature of this operation.
- `perform`: Perform the operation with pre-evaluated arguments.



## ac_runtime::operation::OperationSig

*Struct*

The signature of an operation — its name and parameter types.

Used for introspection and handler matching. Parameter types are
raw schema values in the concrete value representation.

**Generic Parameters:**
- V

**Fields:**
- `name: String` - The operation name (e.g. `"fetch"`, `"read_file"`).
- `params: Vec<(String, V)>` - `(param_name, raw_schema)` pairs, in declaration order.
- `return_schema: Option<V>` - Raw schema value for the return type, if declared.

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &OperationSig<V>) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> OperationSig<V>`



