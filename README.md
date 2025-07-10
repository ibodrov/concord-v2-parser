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
  - [x] `block`
  - [x] `call`
  - [x] `checkpoint`
  - [x] `expr`
  - [x] `form`
  - [x] `if`
  - [x] `log`
  - [x] `logYaml`
  - [x] `parallel`
  - [x] `script`
  - [x] `set`
  - [x] `suspend`
  - [x] `switch`
  - [x] `task`
  - [x] `throw`
  - [x] `try`
