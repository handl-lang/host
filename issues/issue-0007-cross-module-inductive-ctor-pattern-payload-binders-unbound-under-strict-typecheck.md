+++
id = "I-007"
title = "cross-module inductive ctor-pattern payload binders unbound under strict typecheck"
status = "open"
created = "2026-11-28"
priority = "med"
+++

After qualified-ctor support (tool-typecheck-nbe@8ec4a30+), in-module match payload binders work, but ctor patterns on families imported from another module still report unbound binder vars under strict typecheck. Verified while fixing lib-option@5de192c — new Inductive migration was not viable for lib-option because option.test.hnl's some(v) patterns broke. Engine gap in module-boundary ctor_sig propagation.
