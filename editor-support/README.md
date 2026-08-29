# Editor Syntax Highlighting for `.hnl` files

Two layers of support are provided:

## 1. LSP Semantic Tokens (dynamic, plugin-aware)

The `handl-host-lsp` binary is an LSP server that provides
`textDocument/semanticTokens` using the canonical kit's lexer.

### VS Code

1. Build the server and CLI: `cargo build -p handl-host-lsp -p handl-host-cli`
2. Install the extension from `editor-support/vscode/`:
   ```sh
   cd editor-support/vscode
   npm install
   npm run build          # generate grammar + bundle with Vite
   # Option A: package and install
   npm run dev            # builds, packages, and installs the VSIX
   # Option B: reload-on-save dev workflow
   npm run watch          # recompile on save
   # then reload the VS Code window (Developer: Reload Window)
   ```
3. Open any `.hnl` file — the extension auto-launches the LSP server
   from `target/debug/handl-host-lsp`. Override the path with the
   `handl-lsp.server.path` setting if needed.

#### Development

```
editor-support/vscode/
├── src/
│   ├── extension.ts      # entry point (activate/deactivate)
│   ├── server.ts         # server path resolution + client config (testable)
│   └── server.test.ts    # unit tests (vitest)
├── scripts/
│   └── gen-grammar.mjs   # calls hnl gen-grammar to auto-generate TextMate grammar
├── grammars/
│   └── hnl.tmLanguage.json  # auto-generated from the canonical kit
├── vite.config.ts        # Vite SSR bundling config (externalizes vscode)
├── vitest.config.ts      # separate test config (avoids build interference)
├── tsconfig.json         # typecheck config (noEmit, excludes tests)
├── tsconfig.test.json    # test typecheck config (includes tests)
└── package.json
```

```sh
npm test          # run vitest
npm run typecheck # tsc --noEmit on all files
npm run build     # gen-grammar + vite build → dist/extension.js
npm run watch     # vite build --watch (recompile on save)
```

### Neovim

See `neovim/lsp.lua` — requires `nvim-lspconfig` and the `handl-host-lsp`
binary on `$PATH`.

### Helix / Zed / Sublime Text

Any editor with LSP semantic tokens support can connect. Configure the
server command as `handl-host-lsp` for the `hnl` file type.

## 2. TextMate Grammar (auto-generated)

The `hnl.tmLanguage.json` grammar is auto-generated from the canonical
kit's grammar metadata by `npm run gen-grammar` (part of `npm run build`).
It calls `hnl gen-grammar` which uses `host-tooling::generate_textmate`
to produce the grammar from the kit's token plugins.

To regenerate manually:

```sh
cargo build -p handl-host-cli
cd editor-support/vscode && npm run gen-grammar
# or directly:
../../target/debug/hnl gen-grammar grammars/hnl.tmLanguage.json
```

When the kit's plugin composition changes, `npm run build` regenerates
the grammar automatically.

### GitHub Linguist

Drop the generated `hnl.tmLanguage.json` into `.github/` or contribute
it to Linguist for GitHub rendering.
