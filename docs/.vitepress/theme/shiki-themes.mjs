// Green keywords/headings; white function names, literals, and grayish type names.

export const handlDark = {
  name: "handl-dark",
  type: "dark",
  colors: {
    "editor.background": "#2a2a2a",
    "editor.foreground": "#ffffff",
    "editor.lineHighlightBackground": "#363636",
    "editor.selectionBackground": "#0f0",
    "editor.selectionForeground": "#000000",
    "editorCursor.foreground": "#0f0",
    "editorLineNumber.foreground": "#999999",
    "editorLineNumber.activeForeground": "#0f0",
  },
  tokenColors: [
    {
      scope: ["comment"],
      settings: { foreground: "#bbbbbb", fontStyle: "italic" },
    },
    {
      scope: ["string", "string.quoted", "string.template"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["string.regexp"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["constant.character.escape", "constant.character"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["constant.numeric", "number"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["constant.language", "support.constant"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: [
        "keyword",
        "storage.type",
        "storage.modifier",
        "keyword.control",
        "keyword.operator",
        "keyword.other",
      ],
      settings: { foreground: "#0f0", fontStyle: "italic" },
    },
    {
      scope: [
        "keyword.control.flow",
        "keyword.control.return",
        "keyword.control.import",
        "keyword.control.export",
      ],
      settings: { foreground: "#0f0", fontStyle: "italic" },
    },
    {
      scope: [
        "storage.modifier.rust",
        "storage.modifier.mut.rust",
        "keyword.operator.borrow.rust",
        "keyword.operator.borrow.and.rust",
        "keyword.operator.dereference.rust",
      ],
      settings: { foreground: "#0ff" },
    },
    {
      scope: ["variable.language.self.rust", "variable.language.super.rust"],
      settings: { foreground: "#0ff", fontStyle: "italic" },
    },
    {
      scope: ["keyword.operator.key-value.rust"],
      settings: { foreground: "#888888" },
    },
    {
      scope: ["keyword.operator.namespace.rust", "keyword.other.crate.rust"],
      settings: { foreground: "#0ff" },
    },
    {
      scope: ["entity.name.namespace.rust", "entity.name.module.rust"],
      settings: { foreground: "#0ff" },
    },
    {
      scope: ["entity.name.function", "support.function"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["meta.function-call", "meta.method-call"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["entity.name.type"],
      settings: { foreground: "#c5c5c5" },
    },
    {
      scope: ["entity.name.type.trait.rust"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["entity.name.type.struct.rust"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["entity.name.type.rust"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["entity.name.type.impl.rust"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["entity.name.class", "support.class"],
      settings: { foreground: "#c5c5c5", fontStyle: "bold" },
    },
    {
      scope: ["entity.name.type.interface"],
      settings: { foreground: "#c5c5c5", fontStyle: "italic" },
    },
    {
      scope: ["entity.name.type.enum"],
      settings: { foreground: "#c5c5c5" },
    },
    {
      scope: ["entity.name.type.struct", "entity.name.type.union"],
      settings: { foreground: "#c5c5c5" },
    },
    {
      scope: ["entity.name.tag"],
      settings: { foreground: "#b3ff66" },
    },
    {
      scope: ["entity.other.attribute-name"],
      settings: { foreground: "#0f0" },
    },
    {
      scope: ["entity.name.namespace", "entity.name.module"],
      settings: { foreground: "#0f0", fontStyle: "bold" },
    },
    {
      scope: ["support.type", "support.type.primitive"],
      settings: { foreground: "#c5c5c5" },
    },
    {
      scope: ["support.type.builtin", "support.class.builtin"],
      settings: { foreground: "#c5c5c5", fontStyle: "bold" },
    },
    {
      scope: ["support.variable"],
      settings: { foreground: "#0f0" },
    },
    {
      scope: ["variable", "identifier", "meta.definition.variable"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["variable.parameter"],
      settings: { foreground: "#ffffff", fontStyle: "italic" },
    },
    {
      scope: ["variable.language", "variable.this"],
      settings: { foreground: "#0f0" },
    },
    {
      scope: ["variable.other.constant"],
      settings: { foreground: "#ffffff" },
    },
    {
      scope: ["operator", "keyword.operator"],
      settings: { foreground: "#0f0" },
    },
    {
      scope: [
        "punctuation",
        "punctuation.definition",
        "punctuation.separator",
        "punctuation.terminator",
      ],
      settings: { foreground: "#888888" },
    },
    {
      scope: ["invalid", "invalid.illegal", "invalid.deprecated"],
      settings: { foreground: "#ffaa80", fontStyle: "underline" },
    },
    {
      scope: ["markup.heading"],
      settings: { foreground: "#0f0", fontStyle: "bold" },
    },
    {
      scope: ["markup.bold"],
      settings: { foreground: "#0f0", fontStyle: "bold" },
    },
    {
      scope: ["markup.italic"],
      settings: { foreground: "#ffffff", fontStyle: "italic" },
    },
    {
      scope: ["markup.link"],
      settings: { foreground: "#0f0" },
    },
    {
      scope: ["markup.inline.raw"],
      settings: { foreground: "#0f0" },
    },
    {
      scope: ["markup.inserted", "diff.inserted"],
      settings: { foreground: "#0f0" },
    },
    {
      scope: ["markup.deleted", "diff.deleted"],
      settings: { foreground: "#ffaa80" },
    },
    {
      scope: ["diff.changed"],
      settings: { foreground: "#b3ff66" },
    },
    {
      scope: ["source"],
      settings: { foreground: "#ffffff" },
    },
  ],
};
