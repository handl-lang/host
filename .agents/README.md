# Agent setup

Repo-local agent skills live in `skills/`.

Generic, org-wide skills live in the **`handl-lang/automation`** repo
(`skills/<name>/SKILL.md`) — currently `workspace-structure` and `write-adr`.
Link one in when you need it locally:

```bash
ln -s ../../../automation/skills/write-adr .agents/skills/write-adr
```

Repo-specific skills (`ci-cd`, `testing`, `git-conventions`,
`create-package`, `frontend-design`, `writing-documentation-with-diataxis`)
stay here.
