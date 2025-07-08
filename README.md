# concord-v2-parser

A work-in-progress parser for Concord's `runtime-v2` format.
The main goal is to avoid any external dependencies except for `yaml-rust2`.

Status:
- top-level blocks:
  - [x] basic `configuration` parsing
    - [ ] structured parsing (`runtime`, `debug` and other standard parameters)
  - [x] basic `flows` parsing
    - [x] log calls
    - [x] task calls
    - [ ] everything else
  - [x] `forms` parsing
  - [ ] everything else
