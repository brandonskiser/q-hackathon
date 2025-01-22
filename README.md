# Hackathon Project

An AI-powdered CLI for your terminal and editor.

The goal of our project was to explore how a binary with access to LLM-powered capabilities could be used in the command line to recreate common features found across IDE's and other higher-level AI tools. In our case, we designed the API for such a binary by creating a neovim plugin with code-modification and chat capabilities.

## Overview

This project consists of two components:
1. A rust binary called `hackathon`
2. A neovim plugin under `lua/`

## Prerequisites

1. Install Rust: https://www.rust-lang.org/tools/install
2. Set up model access in AWS Bedrock in a personal AWS account.
   The binary is hardcoded to use the model `anthropic.claude-3-haiku-20240307-v1:0` in region `us-west-2`.
3. Set up credentials for calling Bedrock. This can be done with the following script which uses `ada` - see `toolbox install ada`:
```sh
#!/usr/bin/env sh

set -e
AWS_ACCOUNT=YOUR_AWS_ACCOUNT
ROLE="admin"

echo "Refreshing credentials for account ${AWS_ACCOUNT} with role ${ROLE}"
ada credentials update --account="${AWS_ACCOUNT}" --provider=isengard --role="${ROLE}" --once
```
4. (optional) install neovim: https://github.com/neovim/neovim/blob/master/INSTALL.md
Set this directory as a plugin in whatever plugin manager you use.
Using Lazy:
```lua
{
    dir = "~/workplace/hackathon",
    opts = {},
}
```

## Usage

See `cargo run -- --help` for up-to-date options.

## Examples

```sh
cat src/main.rs | cargo run -- code 'generate tests for this file'
```

## Design Notes

h \[options...] <PROMPT>

<PROMPT> is the user prompt

Reads the entirety of stdin until EOF, to use as context for the user prompt. Stdin is expected to be a code block, e.g. from a file.

options:
-f, --file-context                path to a file to use as context
-p, --cursor-position             where the user's cursor is positioned. Formatted as (row,col,[file_path])
-d, --directory-context           path to a directory to use as context

output:
```typescript
type CliOutput = {
    type: 'chat',
    message: string
} | {
    type: 'code',
    message: Array<{
        language: string,
        code: string,
        file_path?: string
    }>
};
```

