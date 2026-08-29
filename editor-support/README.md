# Editor Syntax Highlighting for `.hnl` files

Two layers of support are provided:

## 1. LSP Semantic Tokens (dynamic, plugin-aware)

The `handl-host-lsp` binary is an LSP server that provides
`textDocument/semanticTokens` using the canonical kit's lexer.

### VS Code

1. Build the server: `cargo build -p handl-host-lsp`
2. Install the extension from `editor-support/vscode/`:
   ```sh
   cd editor-support/vscode
   npm install
   # Option A: symlink for development
   ln -s "$(pwd)" ~/.vscode/extensions/handl-lang.handl-lsp-0.1.0
   # Option B: package and install
   npx vsce package && code --install-extension handl-lsp-0.1.0.vsix
   ```
3. Open any `.hnl` file — the extension auto-launches the LSP server
   from `target/debug/handl-host-lsp`. Override the path with the
   `handl-lsp.server.path` setting if needed.

### Neovim

See `neovim/lsp.lua` — requires `nvim-lspconfig` and the `handl-host-lsp`
binary on `$PATH`.

### Helix / Zed / Sublime Text

Any editor with LSP semantic tokens support can connect. Configure the
server command as `handl-host-lsp` for the `hnl` file type.

## 2. TextMate Grammar (static fallback)

Generate a `.tmLanguage.json` from the kit's grammar metadata:

```sh
cargo build -p handl-host-cli
./target/debug/hnl gen-grammar                    # print to stdout
./target/debug/hnl gen-grammar hnl.tmLanguage.json # write to file
```

This produces a TextMate grammar with keywords, operators, strings,
numbers, comments, and delimiters extracted from the kit's token plugins.
When the kit's plugin composition changes, regenerate the grammar to
reflect the new token set.

### GitHub Linguist

Drop the generated `hnl.tmLanguage.json` into `.github/` or contribute
it to Linguist for GitHub rendering.
