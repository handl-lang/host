**handl_lib_serialize > traits**

# Module: traits

## Contents

**Traits**

- [`Decoder`](#decoder) - Decodes input of type `I` into a value of type `T`.
- [`Encoder`](#encoder) - Encodes a value of type `T` into a wire representation.

---

## handl_lib_serialize::traits::Decoder

*Trait*

Decodes input of type `I` into a value of type `T`.

**Methods:**

- `decode`: Decode `input` from the wire format.



## handl_lib_serialize::traits::Encoder

*Trait*

Encodes a value of type `T` into a wire representation.

**Methods:**

- `Output`: The encoded output type, typically `String` or `Vec<u8>`.
- `encode`: Encode `value` into the output format.



