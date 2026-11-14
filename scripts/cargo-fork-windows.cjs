// Tauri still builds and embeds the frontend; only the final Windows link skips PDB generation.
const { spawnSync } = require('node:child_process');
const args = process.argv.slice(2);
if (args[0] === 'build') {
  args[0] = 'rustc';
  const bins = args.indexOf('--bins');
  if (bins >= 0) args.splice(bins, 1, '--bin', 'cockpit-tools');
  args.push('--', '-C', 'link-arg=/DEBUG:NONE');
}
const result = spawnSync('cargo', args, { stdio: 'inherit', env: process.env });
if (result.error) { console.error(result.error.message); process.exit(1); }
process.exit(result.status ?? 1);
