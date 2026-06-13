import { createApp } from "vue";
import "./assets/styles/viui-tokens.css";
import "./styles.css";
import vuetify from "./plugins/vuetify";
import App from "./App.vue";

createApp(App).use(vuetify).mount("#app");
