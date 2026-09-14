// Sample WebIDL fixture covering the supported subset.

[Exposed=Window]
interface Event {
    readonly attribute DOMString type;
    attribute boolean bubbles;
    constructor(DOMString type, optional EventInit eventInitDict);
    undefined stopPropagation();
};

dictionary EventInit {
    boolean bubbles = false;
    boolean cancelable = false;
};

partial interface Event {
    boolean isTrusted();
};

interface mixin Paintable {
    attribute DOMString color;
    undefined paint(DOMString color);
};

namespace console {
    void log(DOMString message);
    void warn(any... data);
};

enum ShadowRootMode { "open", "closed" };

callback FrameRequestCallback = undefined (DOMHighResTimeStamp time);

typedef (Int8Array or Uint8Array) MixedBytes;
