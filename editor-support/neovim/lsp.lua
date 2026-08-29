-- Neovim: add to your lspconfig setup
-- Requires: nvim-lspconfig, built handl-host-lsp binary on PATH

local lspconfig = require('lspconfig')

lspconfig.configs.handl_lsp = {
  default_config = {
    cmd = { 'handl-host-lsp' },
    filetypes = { 'hnl' },
    root_dir = function(fname)
      return lspconfig.util.find_git_ancestor(fname)
    end,
    settings = {},
  },
}

lspconfig.handl_lsp.setup({})
