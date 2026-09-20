// Multiple enums: exercises deterministic ordering of enum_constants —
// they are collected across a HashMap merge, so without an explicit sort
// the emitted order varies between runs.
enum ZetaMode { "a", "b" };
enum AlphaMode { "x", "y" };
enum MidMode { "m", "n" };

interface Iface {
  undefined op();
};
