+++
id = "I-003"
title = "eqsat joint-pass perf regression: layout laws unbounded in Compositional cross_pairs"
status = "open"
created = "2026-11-16"
priority = "high"
+++

eqsat_v0110_perf_check.md: joint widened ruleset (_JOINT_DEFAULT, 219 rules) now covers 158 corpus laws vs 81 at v0.1.6 — the +77 layout-pack laws fire effectively unbounded (only 4/83 carry rule_budgets). Measured: same 16-node joint term 0.35s -> 82-132s, identical extraction cost 6.0. 92% of time in guard/shape check re-eval (2.17M substs); shape memo misses on fresh term identity. Mitigations in report §6: re-mark worst tests slow, budget/exclude layout laws in joint set, engine memo key on (op,attrs,child-shapes), corpus tiering. Affects every consumer of the widened joint pass.
