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
