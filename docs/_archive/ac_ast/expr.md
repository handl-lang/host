**handl_ast > expr**

# Module: expr

## Contents

**Traits**

- [`ExprFactory`](#exprfactory) - Factory for constructing expression nodes.

---

## handl_ast::expr::ExprFactory

*Trait*

Factory for constructing expression nodes.

A `Lexer`/`Parser` or an embedded consumer can implement a custom
factory to attach its own metadata while still returning a standard
`Node<ExprKind, V, M>` tree.

**Methods:**

- `value`
- `call`
- `apply`
- `array`
- `object`
- `index`



