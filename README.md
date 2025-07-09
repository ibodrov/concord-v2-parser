# concord-v2-parser

A work-in-progress parser for Concord's `runtime-v2` format.

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
  - [x] `task`
  - [x] `expr`
  - [x] `script`
  - [x] `call`
  - [ ] everything else
