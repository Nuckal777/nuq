# Nuq
A multi-format frontend to jq.

## Motivation
Based on some recent discussion at work I decided that an other option to process YAML is required. :wink:

## How it works
Converts the input using [serde](https://serde.rs/) to JSON, runs it through `jq` with [jq-rs](https://crates.io/crates/jq-rs), which uses `libjq` (no shell-out) and transform it to the output format with [serde](https://serde.rs/) again.
Should techically work on all formats supported by [serde](https://serde.rs/).
