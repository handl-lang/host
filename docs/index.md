---
# https://vitepress.dev/reference/default-theme-home-page
layout: home

hero:
  name: "Handl Language"
  text: "Docs"
  tagline: Documentation for the handl language, organized by Diátaxis
  actions:
    - theme: brand
      text: Tutorials
      link: /tutorials/
    - theme: brand
      text: How-to guides
      link: /how-to/
    - theme: alt
      text: Explanations
      link: /explanation/
    - theme: alt
      text: Reference
      link: /reference/

features:
  - title: Tutorials
    details: Learn handl by doing — step-by-step lessons for beginners.
    link: /tutorials/getting-started
    linkText: Get started
  - title: How-to guides
    details: Practical recipes for common tasks.
    link: /how-to/
    linkText: How-to guides
  - title: Explanations
    details: Understand the language, runtime, and platform architecture.
    link: /explanation/
    linkText: Read explanations
  - title: Reference
    details: Function-level reference for every lib-* module.
    link: /reference/
    linkText: Browse reference
  - title: Universal plugins
    details: A single `Plugin` model composes libraries, targets, operations, and rules.
    link: /explanations/polyglot-runtimes
    linkText: Read more
  - title: Per-module syntax
    details: Load modules in different syntaxes by dispatching `Parser`, `Lexer`, and `Formatter` rules.
    link: /explanations/polyglot-runtimes
    linkText: Read more
  - title: Self-hosting host
    details: The `Host` itself can eventually be written and spawned from within `handl`.
    link: /explanations/polyglot-runtimes
    linkText: Read more
  - title: Capability tracking
    details: Composition actions like `contribute` and `execute` are language-level effects.
    link: /explanations/polyglot-runtimes
    linkText: Read more
---

## Documentation structure

This documentation follows the [Diátaxis](https://diataxis.fr/) framework, which separates documentation into four purposes:

- **Tutorials** — lessons that help newcomers learn handl.
- **How-to guides** — recipes for solving specific problems.
- **Explanations** — background and conceptual understanding.
- **Reference** — detailed information about functions and APIs.

Use the sidebar or the cards above to navigate.
