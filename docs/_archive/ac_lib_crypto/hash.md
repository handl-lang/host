**handl_lib_crypto > hash**

# Module: hash

## Contents

**Structs**

- [`Md5`](#md5) - `crypto.md5(data: string | array<number>) → string`
- [`Sha256`](#sha256) - `crypto.sha256(data: string | array<number>) → string`

---

## handl_lib_crypto::hash::Md5

*Struct*

`crypto.md5(data: string | array<number>) → string`

Computes the MD5 hash of the input and returns it as a
lowercase hex-encoded string.

MD5 is cryptographically broken and must not be used for security
purposes. It remains useful for checksums, content addressing, and
legacy interoperability.

Examples:
- `crypto.md5("hello")` → `"5d41402abc4b2a76b9719d911017c592"`
- `crypto.md5([104, 101, 108, 108, 111])` → same as above

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Md5`



## handl_lib_crypto::hash::Sha256

*Struct*

`crypto.sha256(data: string | array<number>) → string`

Computes the SHA-256 hash of the input and returns it as a
lowercase hex-encoded string.

The input may be a string (UTF-8 bytes) or an array of integers
in the range 0–255.

Examples:
- `crypto.sha256("hello")` → `"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"`
- `crypto.sha256([104, 101, 108, 108, 111])` → same as above

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Sha256`



