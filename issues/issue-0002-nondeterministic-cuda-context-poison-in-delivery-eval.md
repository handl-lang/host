+++
id = "I-002"
title = "Nondeterministic CUDA context poison in delivery eval"
status = "open"
created = "2026-11-16"
priority = "med"
+++

delivery_prior_v6_result.md run A attempt 1 died to illegal-memory-access during held-out eval on a case set unrelated to arch:t5-small. kit-cuda v0.1.2 fixed the known _GraphReplayModule UAF, but poison still observed — either residual UAF path or another capture-safety gap. Live RL episodes can retrigger; arch-t5-small-cuda must stay --skip-name'd until root-caused. See host/project/reports/delivery_prior_v6_result.md + delivery_v6_corpus.md.
