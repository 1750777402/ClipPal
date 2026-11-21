import { defineConfig, loadEnv } from "vite";
import vue from "@vitejs/plugin-vue";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
  // 读取当前打包模式；默认 local，只有显式商业构建才引入账号和支付组件。
  const env = loadEnv(mode, '.', 'VITE_');
  const edition = env.VITE_EDITION || (mode === 'commercial' ? 'commercial' : mode === 'self-hosted' ? 'self-hosted' : 'local');
  return {
  // 编译期替换成常量，打包器才可以真正删除未启用版本的代码。
  define: { 'import.meta.env.VITE_EDITION': JSON.stringify(edition) },
  plugins: [vue()],
  build: {
    outDir: "../dist",
    // 输出目录只含生成文件；清理旧版本资源，防止商业脚本残留进单机安装包。
    emptyOutDir: true,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
        protocol: "ws",
        host,
        port: 1421,
      }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
};
});
