import { defineConfig } from "vitepress";
import referenceSidebar from "./sidebar-reference.mjs";
import { handlDark } from "./theme/shiki-themes.mjs";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "handl",
  description: "Documentation for the handl language",
  ignoreDeadLinks: true,
  appearance: "dark",
  head: [
    ["link", { rel: "preconnect", href: "https://fonts.googleapis.com" }],
    ["link", { rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "" }],
    [
      "link",
      {
        href: "https://fonts.googleapis.com/css2?family=Source+Code+Pro:ital,wght@0,400;0,600;0,700;1,400&family=Source+Sans+3:ital,wght@0,400;0,600;0,700;1,400&display=swap",
        rel: "stylesheet",
      },
    ],
  ],
  markdown: {
    theme: {
      dark: handlDark,
      light: "github-light",
    },
  },
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    nav: [
      { text: "Home", link: "/" },
      { text: "Tutorials", link: "/tutorials/" },
      { text: "How-to", link: "/how-to/" },
      { text: "Explanation", link: "/explanation/" },
      { text: "Reference", link: "/reference/" },
    ],

    sidebar: {
      "/tutorials/": [
        {
          text: "Tutorials",
          items: [
            { text: "Overview", link: "/tutorials/" },
            { text: "Getting started", link: "/tutorials/getting-started" },
            { text: "Build a host", link: "/tutorials/build-a-host" },
            { text: "Write a plugin", link: "/tutorials/write-a-plugin" },
          ],
        },
      ],
      "/how-to/": [
        {
          text: "How-to guides",
          items: [
            { text: "Overview", link: "/how-to/" },
            { text: "Install hnl", link: "/how-to/install-handl" },
            { text: "Write a handl script", link: "/how-to/write-a-script" },
            { text: "Build a host", link: "/how-to/build-a-host" },
            { text: "Write a plugin", link: "/how-to/write-a-plugin" },
            { text: "Configure options", link: "/how-to/configure-options" },
          ],
        },
      ],
      "/explanation/": [
        {
          text: "Explanations",
          items: [
            { text: "Overview", link: "/explanation/" },
            { text: "What is handl?", link: "/explanation/what-is-handl" },
            { text: "Language overview", link: "/explanation/language-overview" },
            { text: "Runtime", link: "/explanation/runtime" },
            { text: "Platform architecture", link: "/explanation/platform-architecture" },
            { text: "The composable host", link: "/explanation/composable-host" },
            {
              text: "Plugins, rules, and operations",
              link: "/explanation/plugins-rules-operations",
            },
            { text: "Kits and manifests", link: "/explanation/kits-and-manifests" },
            { text: "Options and configuration", link: "/explanation/options-and-configuration" },
            { text: "Security and trust", link: "/explanation/security-and-trust" },
          ],
        },
      ],
      "/reference/": [
        {
          text: "Reference",
          link: "/reference/",
          collapsed: false,
          items: referenceSidebar,
        },
      ],
    },

    search: {
      provider: "local",
    },

    socialLinks: [{ icon: "github", link: "https://github.com/handl-lang" }],
  },
});
