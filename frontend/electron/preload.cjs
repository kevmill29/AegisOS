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
});
