import { h } from "vue";
import DefaultTheme from "vitepress/theme";
import "./style.css";

export default {
  extends: DefaultTheme,
  Layout: () => {
    return h(DefaultTheme.Layout, null, {});
  },
  enhanceApp({ _app, _router, _siteData }) {
    // app-specific logic
  },
};
