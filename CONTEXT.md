# Project Context

This is a personal project for learning to work with coding agents and to become more
comfortable writing idiomatic Rust. Its practical goal is to replace macOS `ls` for the
owner's shell use.

## Direction

The specifications in [`specs/`](./specs/) define the initial behavior and are a starting
point, not a permanent limit. The long-term goal is feature parity with the macOS `ls`
implementation. Use [`specs/man-ls.txt`](./specs/man-ls.txt) as the reference when planning
later additions, and evolve the specifications as the project grows.

## Collaboration

When asked to make a change, implement the complete requested change and then explain the
design and relevant idiomatic Rust choices. The explanation should support the owner's Rust
learning goals without requiring a step-by-step implementation process.
