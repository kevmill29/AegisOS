const { contextBridge, ipcRenderer } = require('electron');

// The renderer's entire view of the outside world. Subscribe-only by design —
// the frontend visualizes agent state, it doesn't command the agent (yet; when
// Phase 2 grows voice/command input, that becomes an explicit, separate channel).
contextBridge.exposeInMainWorld('aegis', {
  onEvent(callback) {
    const handler = (_evt, payload) => callback(payload);
    ipcRenderer.on('aegis:event', handler);
    return () => ipcRenderer.removeListener('aegis:event', handler);
  },
  onLink(callback) {
    const handler = (_evt, payload) => callback(payload);
    ipcRenderer.on('aegis:link', handler);
    return () => ipcRenderer.removeListener('aegis:link', handler);
  },
  // Multi-monitor layout: where the primary display sits in the extended
  // desktop, so the sphere can render on one screen and cloud the rest.
  onLayout(callback) {
    const handler = (_evt, payload) => callback(payload);
    ipcRenderer.on('aegis:layout', handler);
    return () => ipcRenderer.removeListener('aegis:layout', handler);
  },
  // Terminal: intentionally command-granular rather than a raw PTY handle, so
  // the renderer can only run what the user typed into the terminal UI.
  onToggleTerminal(callback) {
    const handler = () => callback();
    ipcRenderer.on('aegis:toggle-terminal', handler);
    return () => ipcRenderer.removeListener('aegis:toggle-terminal', handler);
  },
  execCommand(execId, command) {
    ipcRenderer.send('terminal:exec', { execId, command });
  },
  killCommand(execId) {
    ipcRenderer.send('terminal:kill', { execId });
  },
  onTerminalData(callback) {
    const handler = (_evt, payload) => callback(payload);
    ipcRenderer.on('terminal:data', handler);
    return () => ipcRenderer.removeListener('terminal:data', handler);
  },
  // GUI installer (live ISO only — installerCapable() is false on installed
  // systems and in dev, and the UI hides itself). start() carries the user's
  // answers incl. password over IPC + stdin, never argv.
  installerCapable() {
    return ipcRenderer.invoke('installer:capable');
  },
  installerListDisks() {
    return ipcRenderer.invoke('installer:list-disks');
  },
  installerStart(answers) {
    ipcRenderer.send('installer:start', answers);
  },
  installerReboot() {
    ipcRenderer.send('installer:reboot');
  },
  onInstallerProgress(callback) {
    const handler = (_evt, payload) => callback(payload);
    ipcRenderer.on('installer:progress', handler);
    return () => ipcRenderer.removeListener('installer:progress', handler);
  },
  // Called once the renderer has attached its listeners: main replays the
  // current link state + last Hello. Push-only delivery loses the startup
  // events on slow targets, where the socket connects before the first
  // paint (and React's effects) have happened.
  ready() {
    ipcRenderer.send('aegis:ui-ready');
  },
});
