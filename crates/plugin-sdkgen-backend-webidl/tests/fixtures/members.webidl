[Exposed=*]
interface Widget {
  constructor(USVString name);

  static Widget? find(USVString name);
  static boolean canFind(USVString name);

  stringifier attribute USVString href;
  readonly attribute USVString origin;

  iterable<USVString, USVString>;
  stringifier;
};
