# Hackathon Project

An AI-powdered CLI for your terminal and editor.

## Examples

cat src/main.rs | h 'generate tests for this file'

## Usage

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


## Examples 

```sh
h -f src/main.rs -f src/lib.rs 'add another option to subtract numbers'
h -f src/main.rs,src/lib.rs 'jsdlfjdslk'
h -d src 'jsdlfjdslk'
cat src/main.rs | h 'generate tests for this file'
```

## Presentation Steps

1. Give overview of the purpose of our project
"The goal of our project was to explore how a binary with access to LLM-powered capabilities could be used
in the command line to recreate common features found across IDE's and other higher-level AI tools."
"In our case, we designed the API for such a binary by creating a neovim plugin with code-modification and
chat capabilities."
2. Show picture of how it works
3. Demonstrate `:QCode`
    1. In ./examples/math/: `:QCode 'add code for subtracting, multiplying, and dividing numbers'` - apply
    2. `:QCode 'add comments explaining each function'` - exit out if not doc comments
    3. `:QCode 'add doc comments explaining each function'` - apply
    <!-- 4. `:QCode 'make these math functions generic'` - apply -->
4. Demonstrate `:QChat`
    1. In ./examples/data_structures/:
        `:Qsf` and select all the data structure files
        in `QChat` 'can you provide an example of how to use these data structures?'
        'How would you use these examples as tests for each file?'

