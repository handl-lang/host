+++
id = "I-005"
title = "typecheck model: effects checking is NBE-only (synth_handle/synth_perform/check_call perform missing)"
status = "open"
created = "2026-11-25"
priority = "med"
+++

Found by M4 effects-as-modes assessment (project 8773d281, design/effects-as-modes.md). tool-typecheck-handl's typecheck_model.hnl judgment table has no synth_handle/synth_perform and check_call lacks the perform branch — type.handle degrades to 'unknown judgment op' warning on the model/adapter path. Effects checking currently works only via NbeTypecheckEngine. Prerequisite parity gap for any effects-adjacent work (M4, I-004 check_call). See design/effects-as-modes.md blockers section.
