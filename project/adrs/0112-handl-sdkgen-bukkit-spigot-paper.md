# ADR-0112: Handl SDK Generator — Bukkit, Spigot, Paper Backend

## Status

Proposed.

## Context

[ADR-0107 — Handl SDK Generator](./0107-handl-sdk-generator.md) established a
backend-agnostic `SdkgenModule` IR, and [ADR-0111](./0111-handl-sdkgen-jvm.md)
proposes a generic JVM backend. Bukkit, Spigot, Paper, and similar Minecraft
server platforms are large Java APIs. They should be supported as a specialized
JVM backend rather than as a separate language target.

## Decision

Build `plugin-sdkgen-backend-bukkit` on top of the JVM backend
(`plugin-sdkgen-backend-jvm`).

- The Bukkit backend accepts a manifest of Bukkit / Spigot / Paper class
  mappings (e.g. `org.bukkit.Server`, `org.bukkit.entity.Player`) and emits
  Handl ports and adapters for them.
- It reuses the JVM IR and the `jvm` client package.
- It adds event-hook generation so Bukkit events can be exposed as Handl graded
  arrows.
- A `.sdkgen.yaml` file selects the target platform (`bukkit` / `spigot` /
  `paper`) and version so the correct class names and method signatures are
  used.
- The first target is Spigot / Paper on Linux x86_64 because that is the most
  common server stack. Windows / macOS server support is out of scope.

## Consequences

- Handl can author Minecraft server plugins in Handl while reusing the full
  Bukkit / Spigot / Paper API.
- The JVM backend (ADR-0111) is a hard prerequisite.
- Configuration (`.sdkgen.yaml`) becomes more important because the class
  inventory is large and versioned.
- This work has lower priority than the generic JVM backend but higher priority
  than Windows / macOS C support (ADR-0110).

## References

- [ADR-0107 — Handl SDK Generator](./0107-handl-sdk-generator.md)
- [ADR-0111 — Handl SDK Generator: JVM Backend](./0111-handl-sdkgen-jvm.md)
- `crates/plugin-sdkgen`
