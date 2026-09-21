// Supplementary spec: supplies `MissingMixin`, which `includes.webidl`'s
// `Request includes MissingMixin` references cross-spec. Only mixin
// definitions are consumed — this interface must not be emitted.

interface mixin MissingMixin {
  readonly attribute boolean extra;
};

interface SupplementaryOnly {
  readonly attribute boolean shouldNotAppear;
};

// Graft target for `Foreign includes Body` in includes.webidl — the
// interface is foreign to the primary spec, so the augmented record
// emits in the primary package.
interface Foreign {
  readonly attribute boolean ownMember;
};
