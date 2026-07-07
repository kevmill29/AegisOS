// Electron main process: owns the window and the TCP link to the core agent.
// The renderer never touches the network — events arrive via the preload bridge,
// which keeps contextIsolation on and nodeIntegration off.
const { app, BrowserWindow, globalShortcut, ipcMain } = require('electron');
const { spawn } = require('node:child_process');
const net = require('node:net');
const path = require('node:path');

const AGENT_HOST = '127.0.0.1';
const AGENT_PORT = 48620;
const RECONNECT_DELAY_MS = 2000;

// On the real Aegis target this runs as the sole fullscreen surface under cage;
// during development a normal window is far less hostile to iterate against.
const KIOSK = process.env.AEGIS_KIOSK === '1';

let win = null;

function createWindow() {
  win = new BrowserWindow({
    width: 1280,
    height: 800,
    backgroundColor: '#000005',
    fullscreen: KIOSK,
    frame: !KIOSK,
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  win.loadFile(path.join(__dirname, '..', 'dist', 'index.html'));
}

function sendToRenderer(channel, payload) {
  if (win && !win.isDestroyed()) {
    win.webContents.send(channel, payload);
  }
}

// Line-buffered NDJSON client with dumb-but-sufficient reconnect: the daemon may
// start after us, restart under systemd, or the WSL VM may drop — in every case
// the right behavior is identical (keep retrying quietly, resync from the Hello
// event on reconnect), so there's no backoff cleverness here.
function connectToAgent() {
  const socket = net.connect(AGENT_PORT, AGENT_HOST);
  let buffer = '';

  socket.on('connect', () => {
    console.log('[aegis] agent link established');
    sendToRenderer('aegis:link', { connected: true });
  });

  socket.on('data', (chunk) => {
    buffer += chunk.toString('utf8');
    let newlineAt;
    while ((newlineAt = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, newlineAt).trim();
      buffer = buffer.slice(newlineAt + 1);
      if (!line) continue;
      try {
        console.log('[aegis] event:', line);
        sendToRenderer('aegis:event', JSON.parse(line));
      } catch {
        console.warn('unparseable event line:', line);
      }
    }
  });

  const retry = () => {
    sendToRenderer('aegis:link', { connected: false });
    setTimeout(connectToAgent, RECONNECT_DELAY_MS);
  };
  socket.on('error', () => {}); // 'close' always follows 'error'; retry once, there
  socket.on('close', retry);
}

// ---- Terminal backing: real bash, streamed output ----
// On the Linux target this is native bash; during Windows-side dev it routes
// into the same WSL distro the daemon runs in, so `ls /sys/fs/cgroup` etc.
// show the machine the agent actually lives on. One command per exec (a REPL,
// not a full PTY — no ncurses apps; that upgrade is node-pty, deferred until
// the Linux build where it compiles painlessly).
function bashInvocation(command) {
  if (process.platform === 'win32') {
    return { bin: 'wsl.exe', args: ['-d', 'Ubuntu', '-e', 'bash', '-lc', command] };
  }
  return { bin: 'bash', args: ['-lc', command] };
}

const running = new Map(); // execId -> child process

ipcMain.on('terminal:exec', (event, { execId, command }) => {
  const { bin, args } = bashInvocation(command);
  const child = spawn(bin, args, { windowsHide: true });
  running.set(execId, child);

  const send = (kind, data) =>
    event.sender.send('terminal:data', { execId, kind, data });

  child.stdout.on('data', (d) => send('stdout', d.toString('utf8')));
  child.stderr.on('data', (d) => send('stderr', d.toString('utf8')));
  child.on('close', (code) => {
    running.delete(execId);
    send('exit', String(code ?? -1));
  });
  child.on('error', (err) => {
    running.delete(execId);
    send('stderr', `failed to start shell: ${err.message}\n`);
    send('exit', '-1');
  });
});

ipcMain.on('terminal:kill', (_event, { execId }) => {
  running.get(execId)?.kill();
});

app.whenReady().then(() => {
  createWindow();
  connectToAgent();

  // Headless verification of the terminal's exec path: runs the same
  // bashInvocation the UI uses and echoes the result to stdout, where a test
  // harness can assert on it. No-op unless explicitly requested via env.
  if (process.env.AEGIS_TERM_SELFTEST === '1') {
    const { bin, args } = bashInvocation('echo aegis-terminal-selftest-$((6*7))');
    const child = spawn(bin, args, { windowsHide: true });
    child.stdout.on('data', (d) => console.log('[selftest stdout]', d.toString().trim()));
    child.on('close', (code) => console.log('[selftest exit]', code));
  }

  // The kiosk has no desktop environment to own this shortcut, so the app
  // registers it globally. Renderer also listens for the same chord as a
  // fallback for dev, where another app may have claimed the global hook.
  globalShortcut.register('Control+Alt+T', () => {
    sendToRenderer('aegis:toggle-terminal', {});
  });
});

app.on('will-quit', () => {
  globalShortcut.unregisterAll();
  for (const child of running.values()) child.kill();
});

app.on('window-all-closed', () => {
  app.quit();
});
