// `Foreign` is declared only in includes-extra.webidl; this spec just
// references it as an includes target.

interface mixin Body {
  readonly attribute boolean bodyUsed;
};

Foreign includes Body;
