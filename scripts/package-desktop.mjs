// 个人桌面端打包入口，由 frontend/package.json 中的 npm 脚本调用。
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const [edition, ...options] = process.argv.slice(2);
if (!['local', 'self-hosted'].includes(edition)) {
  console.error('请选择 local 或 self-hosted 打包版本。');
  process.exit(1);
}

const repo = fileURLToPath(new URL('../', import.meta.url));
const tauriCli = fileURLToPath(new URL('../frontend/node_modules/@tauri-apps/cli/tauri.js', import.meta.url));
const personalConfig = fileURLToPath(new URL('../src-tauri/tauri.personal.json', import.meta.url));
const args = ['build', '--config', personalConfig];
const cargoArgs = ['--locked'];

// 和 Cargo 入口共用 Tauri 配置，由构建钩子执行前端构建。
if (edition === 'self-hosted') {
  const selfHostedConfig = fileURLToPath(new URL('../src-tauri/tauri.self-hosted.json', import.meta.url));
  args.push('--config', selfHostedConfig);
  cargoArgs.push('--no-default-features');
}

const result = spawnSync(process.execPath, [tauriCli, ...args, ...options, '--', ...cargoArgs], {
  cwd: repo,
  stdio: 'inherit',
});
if (result.error) console.error(result.error.message);
process.exitCode = result.status ?? 1;
