import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import { useSettingsStore } from './stores/settings';
import './styles/global.css';

const app = createApp(App);
app.use(createPinia());

// 启动时加载设置并应用到 DOM，避免主题/字体闪烁
const settings = useSettingsStore();
settings.load();
settings.applyDom();

app.mount('#app');
