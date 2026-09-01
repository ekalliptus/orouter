// Entry point: fonts → theme stylesheets → app. Mirrors react-web's load
// order so both SPAs share the exact 9Router design tokens.
import "@fontsource-variable/inter";
import "@fontsource/gochi-hand";
import "@fontsource/patrick-hand";
import "material-symbols/outlined.css";
import "@/styles/theme.css";
import "@/styles/kiddraw.css";

import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";

createApp(App).use(router).mount("#app");
