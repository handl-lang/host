// `A includes B;` grafts the mixin's members onto the target interface.

interface mixin Body {
  readonly attribute boolean bodyUsed;
  Promise<Blob> blob();
  Promise<USVString> text();
};

interface Request {
  readonly attribute USVString url;
};

interface Response {
};

Request includes Body;
Response includes Body;
Ghost includes Body;
Request includes MissingMixin;
