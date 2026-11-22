// 个人桌面端打包入口，由 frontend/package.json 中的 npm 脚本调用。
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const [edition, ...options] = process.argv.slice(2);
if (!['local', 'self-hosted'].includes(edition)) {
  console.error('请选择 local 或 self-hosted 打包版本。');
  process.exit(1);
}

const repo = fileURLToPath(new URL('../', import.meta.url));
const frontend = fileURLToPath(new URL('../frontend/', import.meta.url));
const tauriCli = fileURLToPath(new URL('../frontend/node_modules/@tauri-apps/cli/tauri.js', import.meta.url));
const personalConfig = fileURLToPath(new URL('../src-tauri/tauri.personal.json', import.meta.url));

/** 等待当前打包步骤结束，失败时停止，防止拿旧的前端产物继续打包。 */
function runNode(script, args, cwd, env = process.env) {
  const result = spawnSync(process.execPath, [script, ...args], { cwd, env, stdio: 'inherit' });
  if (result.error) console.error(result.error.message);
  if (result.status !== 0) process.exit(result.status ?? 1);
}

// 前端和 Rust 使用同一个版本参数，避免本机已有的 VITE_EDITION 混入其他版本代码。
const viteMode = edition === 'local' ? 'production' : edition;
runNode(process.env.npm_execpath, ['run', 'build', '--', '--mode', viteMode], frontend, {
  ...process.env,
  VITE_EDITION: edition,
});

// 使用 npm 锁文件中的 Tauri CLI。每次只启用所选产品版本，按 Cargo.lock 编译。
runNode(tauriCli, [
  'build', '--config', personalConfig, '--features', edition, ...options,
  '--', '--no-default-features', '--locked',
], repo);
