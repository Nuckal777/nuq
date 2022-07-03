# Nuq

![Build Status](https://img.shields.io/github/workflow/status/Nuckal777/nicator/test)
[![Coverage Status](https://coveralls.io/repos/github/Nuckal777/nuq/badge.svg?branch=master)](https://coveralls.io/github/Nuckal777/nuq?branch=master)

A multi-format frontend to jq.

## Motivation
Based on some recent discussion at work I decided that an other option to process YAML is required. :wink:

To my knowledge there are two common options for that:
- [yq](https://github.com/mikefarah/yq), which uses Golang and has its interpreter with differs from jq's interpreter.
- [yq](https://github.com/kislyuk/yq), which uses python to convert YAML to JSON and shells out to jq after. Therefore it is slower than native implementations.

`nuq` should have decent speed and can execute all programs accepted by jq due to calling libjq directly.

## Usage
```
nuq [OPTIONS] <PROGRAM> [FILES]...

ARGS:
    <PROGRAM>     Jq program to execute
    <FILES>...    Input files, stdin if omitted

OPTIONS:
    -i, --input-format <INPUT_FORMAT>
            Input format, will be guessed by extension if omitted

    -o, --output-format <OUTPUT_FORMAT>
            Output format, if omitted will return whatever libjq produces

    -r, --raw
            If jq outputs a JSON string only output contained plain text. This post-processes the jq
            output, so it may not behave the same as "jq -r"
```

## How it works
Converts the input using [serde](https://serde.rs/) to JSON, runs it through `jq` with [jq-rs](https://crates.io/crates/jq-rs), which uses `libjq` (no shell-out) and transform it to the output format with [serde](https://serde.rs/) again.
Should techically work on all formats supported by [serde](https://serde.rs/).
