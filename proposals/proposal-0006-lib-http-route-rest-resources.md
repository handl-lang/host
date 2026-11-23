+++
id = "P-006"
title = "lib-http-route: resources()/resource() REST generators"
status = "done"
created = "2026-11-23"
priority = "med"
labels = ["http","routing"]
task = "T-119"
+++

# P-006 — lib-http-route: `resources()`/`resource()` REST generators

***Status:** approved · **Repo:** `lib-http-route` · **Task:** T-119
**Depends on:** v2 route maps (landl); pairs naturally with P-005 patterns

## Motivation

Rails/Remix convention: a resource name expands to the canonical REST
route table — the most common server shape, currently hand-written per
app. Pure sugar over route maps; zero new machinery.

## API

```handl
resources("photos", {
  index:   get("/photos"),
  show:    get("/photos/:id"),
  create:  post("/photos"),
  edit:    get("/photos/:id/edit"),
  update:  put("/photos/:id"),       -- or patch, see below
  destroy: delete("/photos/:id"),
})

resource("profile", { ... })          -- singular: no :id segment
```

Variants:

- `only: ["index","show"]` / `except: ["destroy"]` — subset selection
- `param: "slug"` — rename `:id` → `:slug`
- `path: "/admin/photos"` — mount prefix
- `update` emits `PUT`+`PATCH` on the same handler by default (HTML-form
  reality); `methods: {update: "patch"}` to pin

## Semantics

- Returns a **route-map fragment** (same shape `route()` consumes) —
  merges into named route maps: `route({photos: resources("photos",h), ...})`
- Handlers may be a single arrow (all actions) or a record
  `{index: f, show: f, ...}`; missing actions → 405 via v2 machinery
- `href()` works unchanged: `href(photos.show, {id: 7})` → `/photos/7`
- `form()` pairing (v2) composes: `new`/`edit` GET pages pair with
  `create`/`update` POSTs at the same URL

## Why a lib function, not syntax

Handl has no macro layer in `.hnl` libs; a plain function returning route
records is the idiomatic expansion — inspectable, composable, testable.

## Tests

- Each action emits correct method+pattern; `only`/`except`/`param`/
  `path` honored; singular `resource` drops `:id`
- Merge into route map; `href` round-trip per action; 405 on unhandled
- PUT+PATCH dual emission; `methods` override
