// 开发启动器统一选择前后端版本。日常开发沿用安装版的数据目录。
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const [edition = 'self-hosted', ...options] = process.argv.slice(2);
if (!['local', 'self-hosted'].includes(edition)) {
  console.error('开发版本只能选择 local 或 self-hosted。');
  process.exit(1);
}
const repo = fileURLToPath(new URL('../', import.meta.url));
const cli = fileURLToPath(new URL('../frontend/node_modules/@tauri-apps/cli/tauri.js', import.meta.url));
const child = spawn(process.execPath, [cli, 'dev', '--features', edition, ...options,
  '--', '--no-default-features', '--locked'], {
  cwd: repo,
  env: { ...process.env, VITE_EDITION: edition },
  stdio: 'inherit',
});
child.on('error', error => { console.error(error.message); process.exitCode = 1; });
child.on('exit', code => { process.exitCode = code ?? 1; });
