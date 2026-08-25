import { defineConfig } from "vitepress";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "Handl",
  description: "Documentation for Handl",
  // Existing docs reference ./DIALECT which is not present yet
  ignoreDeadLinks: true,
  appearance: true,
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    nav: [
      { text: "Home", link: "/" },
      { text: "Examples", link: "/markdown-examples" },
      { text: "API", link: "/api/" },
      { text: "Reference", link: "/reference/" },
    ],

    sidebar: [
      {
        text: "Examples",
        items: [
          { text: "Markdown Examples", link: "/markdown-examples" },
          { text: "Runtime API Examples", link: "/api-examples" },
        ],
      },
      {
        text: "API",
        items: [{ text: "Crate Index", link: "/api/" }],
      },
      {
        text: "Reference",
        link: "/reference/",
        collapsed: false,
        items: [
          { text: "Archive", link: "/reference/lib-archive" },
          { text: "Archive (Tar)", link: "/reference/lib-archive-tar" },
          { text: "Base64", link: "/reference/lib-base64" },
          { text: "Collections", link: "/reference/lib-collections" },
          { text: "Compress", link: "/reference/lib-compress" },
          { text: "Compress (Gzip)", link: "/reference/lib-compress-gzip" },
          { text: "Crypto", link: "/reference/lib-crypto" },
          { text: "DateTime", link: "/reference/lib-datetime" },
          { text: "Encoding", link: "/reference/lib-encoding" },
          { text: "Hex", link: "/reference/lib-hex" },
          { text: "HTTP Client", link: "/reference/lib-http-client" },
          { text: "HTTP Client (Std)", link: "/reference/lib-http-client-std" },
          { text: "HTTP Server", link: "/reference/lib-http-server" },
          { text: "HTTP Server (Std)", link: "/reference/lib-http-server-std" },
          { text: "Intl", link: "/reference/lib-intl" },
          { text: "IP", link: "/reference/lib-ip" },
          { text: "JSON", link: "/reference/lib-json" },
          { text: "KV", link: "/reference/lib-kv" },
          { text: "KV (Hashmap)", link: "/reference/lib-kv-hashmap" },
          { text: "OS", link: "/reference/lib-os" },
          { text: "OS (Std)", link: "/reference/lib-os-std" },
          { text: "OTLP", link: "/reference/lib-otlp" },
          { text: "OTLP (Std)", link: "/reference/lib-otlp-std" },
          { text: "Parse", link: "/reference/lib-parse" },
          { text: "Random", link: "/reference/lib-rand" },
          { text: "Serialize", link: "/reference/lib-serialize" },
          { text: "Stats", link: "/reference/lib-stats" },
          { text: "Test", link: "/reference/lib-test" },
          { text: "TOML", link: "/reference/lib-toml" },
          { text: "Unicode", link: "/reference/lib-unicode" },
          { text: "URL (RFC 3986)", link: "/reference/lib-url-rfc" },
          { text: "URL (WHATWG)", link: "/reference/lib-url-whatwg" },
          { text: "Utils", link: "/reference/lib-utils" },
          { text: "WebSocket", link: "/reference/lib-websocket" },
          { text: "YAML", link: "/reference/lib-yaml" },
        ],
      },
    ],

    search: {
      provider: "local",
    },

    socialLinks: [{ icon: "github", link: "https://github.com/handl-lang" }],
  },
});
