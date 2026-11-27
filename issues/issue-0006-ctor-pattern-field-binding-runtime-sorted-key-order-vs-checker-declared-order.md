+++
id = "I-006"
title = "ctor-pattern field binding: runtime sorted-key order vs checker declared order"
status = "open"
created = "2026-11-27"
priority = "high"
+++

Runtime binds ctor-pattern fields in sorted record-key order; typechecker uses declared order. PErr(pos,msg) patterns silently swap at runtime (found during lib-json-handl fix, lib-json-handl@31db25b). Any positional ctor pattern whose field names are not alphabetical is wrong at runtime. Align runtime to declared order (or document+enforce sorted decls).
