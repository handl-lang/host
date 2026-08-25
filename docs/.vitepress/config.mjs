import { defineConfig } from "vitepress";
import referenceSidebar from "./sidebar-reference.mjs";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "Handl",
  description: "Documentation for the handl language",
  ignoreDeadLinks: true,
  appearance: true,
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
