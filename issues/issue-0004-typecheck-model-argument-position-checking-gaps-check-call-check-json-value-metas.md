+++
id = "I-004"
title = "typecheck model: argument-position checking gaps (check_call, check_json_value metas)"
status = "open"
created = "2026-11-25"
priority = "med"
+++

Found by dependent_structure repair (tool-typecheck-handl 1519a15). Two latent soundness gaps in the .hnl model: (1) nested ctor applications in argument position dispatch to unimplemented check_call judgment — inner synthesized type never unified against outer domain, leaving index metas unsolved; (2) check_json_value returns ok for is_meta(expected) without solving the meta, so literal args leave element metas open. Tests must route ctor args through let-bound vars for real checking. See tests/dependent_structure.rs comments.
