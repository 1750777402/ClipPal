import { createApp } from "vue";
import App from "./App.vue";
import "./assets/styles/global.css";
import "./assets/icons/iconfont/iconfont.css";
import "./assets/styles/highlight.css";
import './styles/responsive.css';

createApp(App).mount("#app");

// 全局禁用鼠标右键
document.addEventListener('contextmenu', function (e) {
    e.preventDefault();
});
