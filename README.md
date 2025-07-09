# concord-v2-parser

A work-in-progress parser for Concord's `runtime-v2` format.
The main goal is to avoid any external dependencies except for `yaml-rust2`.

Status:
- top-level blocks:
  - [x] basic `configuration` parsing
    - [ ] structured parsing (`runtime`, `debug` and other standard parameters)
  - [x] basic `flows` parsing
  - [x] basic `forms` parsing
    - [ ] structured parsing of field options
  - [ ] `triggers`
  - [ ] `resources`
  - [ ] `imports`
  - [ ] `publicFlows`
  - [ ] everything else
- flow steps:
  - [x] `log`
  - [x] `throw`
  - [x] basic `task` calls
    - [x] `name`
    - [x] `in`
    - [x] `out`
    - [x] `error`
    - [x] `ignoreErrors`
    - [x] `loop`
    - [x] `meta`
    - [x] `retry`
  - [ ] everything else
