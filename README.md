# Nuq

![Build Status](https://img.shields.io/github/workflow/status/Nuckal777/nicator/test)
[![Coverage Status](https://coveralls.io/repos/github/Nuckal777/nuq/badge.svg?branch=master)](https://coveralls.io/github/Nuckal777/nuq?branch=master)

A multi-format frontend to jq.

## Motivation
Based on some recent discussion at work I decided that an other option to process YAML is required. :wink:

To my knowledge there are two common options for that:
- [yq](https://github.com/mikefarah/yq), which uses Golang and has its interpreter with differs from jq's interpreter.
- [yq](https://github.com/kislyuk/yq), which uses python to convert YAML to JSON and shells out to jq after. Therefore it is slower than native implementations.

`nuq` should have decent speed and can execute all programs accepted by jq due to calling libjq directly. To be fair `nuq` has to parse JSON input streams containing multiple documents to split them before they can be handed off to libjq, so a straight `jq` should be faster. :poop:

## Usage
```
A multi-format frontend for jq

Usage: nuq [OPTIONS] <PROGRAM> [FILES]...

Arguments:
  <PROGRAM>   Jq program to execute
  [FILES]...  Input files, stdin if omitted

Options:
  -i, --input-format <INPUT_FORMAT>
          Input format, will be guessed by extension if omitted [possible values: json, yaml, ron, toml]
  -o, --output-format <OUTPUT_FORMAT>
          Output format, if omitted will return whatever libjq produces. Toml output may require reordering the input [possible values: json, yaml, ron, toml]
  -r, --raw
          If jq outputs a JSON string only output contained plain text. This post-processes the jq output, so it may not behave the same as "jq -r"
      --slurp
          Concatenate all input files into a JSON array before processing it with jq
```

## How it works
Converts the input using [serde](https://serde.rs/) to JSON, runs it through `jq` with [jq-rs](https://crates.io/crates/jq-rs), which uses `libjq` (no shell-out) and transform it to the output format with [serde](https://serde.rs/) again.
Should techically work on all formats supported by [serde](https://serde.rs/).
