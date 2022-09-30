# Knurdy

A minimal, low-copy, very opinionated conversion layer from the [kdl](https://crates.io/crates/kdl) library
to [serde](https://crates.io/crates/serde).

This is mainly intended for Dialga, my crate for a Caves of Qud-like blueprint instatiation system, which
in turn is intended for Palkia. 

There were shortcomings for me with all the extant KDL libraries for Rust:

- Knuffel is very powerful, but rolls its own deserialization system and macros instead of using Serde.
  (Which is appropriate for what it's trying to do, granted; KDL doesn't map all too well to Serde's data model.)
  It stores the AST in memory, but requires also storing the span of text it was parsed from, and I wanted just
  pure ASTs sitting around.
- [Kaydle](https://crates.io/crates/kaydle) is a KDL library that uses Serde, but isn't as feature-complete as Knuffel
  and doesn't store the AST in memory.

## Usage

Call `knurdy::deserialize_node`, or directly use the `KdlNodeDeser`.
