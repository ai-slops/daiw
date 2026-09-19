const { app, BrowserWindow, Menu } = require('electron');
const { spawn } = require('node:child_process');
const { join } = require('node:path');
const http = require('node:http');

let service;
const port = process.env.DAIW_DESKTOP_PORT || '3333';
const origin = `http://127.0.0.1:${port}`;

function ready() {
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const attempt = () => {
      const request = http.get(`${origin}/api/health`, (response) => {
        response.resume();
        if (response.statusCode === 200) resolve();
        else retry();
      });
      request.on('error', retry);
      request.setTimeout(1000, () => request.destroy());
    };
    const retry = () => Date.now() - started > 120000 ? reject(new Error('The local core service did not start. Run npm run core:build and npm run build.')) : setTimeout(attempt, 300);
    attempt();
  });
}

app.whenReady().then(async () => {
  const root = join(__dirname, '..');
  service = spawn(process.execPath, [join(root, 'app/server.mjs')], {
    cwd: root,
    env: { ...process.env, ELECTRON_RUN_AS_NODE: '1', HOST: '127.0.0.1', PORT: port },
    stdio: 'inherit',
  });
  await ready();
  const window = new BrowserWindow({
    title: 'DAIW Studio', width: 1500, height: 960, minWidth: 760, minHeight: 600,
    backgroundColor: '#151718',
    webPreferences: { contextIsolation: true, sandbox: true, nodeIntegration: false },
  });
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.on('will-navigate', (event, target) => {
    if (new URL(target).origin !== origin) event.preventDefault();
  });
  window.webContents.session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  Menu.setApplicationMenu(Menu.buildFromTemplate([
    { label: 'DAIW', submenu: [{ role: 'quit' }] },
    { role: 'editMenu' },
    { role: 'viewMenu' },
    { role: 'windowMenu' },
  ]));
  await window.loadURL(origin);
}).catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : 'Desktop startup failed'}\n`);
  app.quit();
});
app.on('window-all-closed', () => app.quit());
app.on('before-quit', () => service?.kill());
