// Supplementary spec: supplies `MissingMixin`, which `includes.webidl`'s
// `Request includes MissingMixin` references cross-spec. Only mixin
// definitions are consumed — this interface must not be emitted.

interface mixin MissingMixin {
  readonly attribute boolean extra;
};

interface SupplementaryOnly {
  readonly attribute boolean shouldNotAppear;
};
