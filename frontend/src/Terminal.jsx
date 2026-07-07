import React, { useEffect, useRef, useState } from 'react';

// Terminal overlay, toggled by Ctrl+Alt+T (global shortcut in Electron main,
// plus the renderer keydown fallback in App). A command REPL over real bash —
// each Enter spawns `bash -lc <line>` in the main process and streams output
// back. Voice-open is a planned second trigger for the same toggle event once
// a local STT engine (whisper.cpp) lands; the mic capture pipeline it needs
// already exists in audio.js.

const styles = {
  overlay: {
    position: 'absolute',
    left: '50%',
    bottom: '12vh',
    transform: 'translateX(-50%)',
    width: 'min(860px, 92vw)',
    height: '42vh',
    background: 'rgba(2, 10, 24, 0.88)',
    border: '1px solid rgba(51, 214, 255, 0.35)',
    borderRadius: '8px',
    boxShadow: '0 0 32px rgba(51, 214, 255, 0.15)',
    backdropFilter: 'blur(6px)',
    display: 'flex',
    flexDirection: 'column',
    fontFamily: "'Cascadia Mono', 'Consolas', monospace",
    fontSize: '13px',
    zIndex: 10,
  },
  header: {
    padding: '6px 12px',
    color: '#33d6ff',
    letterSpacing: '0.25em',
    textTransform: 'uppercase',
    fontSize: '10px',
    borderBottom: '1px solid rgba(51, 214, 255, 0.2)',
    flexShrink: 0,
  },
  scrollback: {
    flex: 1,
    overflowY: 'auto',
    padding: '8px 12px',
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-word',
  },
  inputRow: {
    display: 'flex',
    alignItems: 'center',
    padding: '6px 12px',
    borderTop: '1px solid rgba(51, 214, 255, 0.2)',
    flexShrink: 0,
  },
  prompt: { color: '#33d6ff', marginRight: '8px', flexShrink: 0 },
  input: {
    flex: 1,
    background: 'transparent',
    border: 'none',
    outline: 'none',
    color: '#d8f6ff',
    font: 'inherit',
  },
};

const lineColor = { cmd: '#33d6ff', stdout: '#d8f6ff', stderr: '#ff9a6b', exit: '#5a708f' };

let nextExecId = 1;

export default function Terminal({ visible, onRequestClose }) {
  const [lines, setLines] = useState([
    { kind: 'exit', text: 'aegis terminal — real bash, one command per line. ctrl+alt+t to hide.' },
  ]);
  const [input, setInput] = useState('');
  const [busyExecId, setBusyExecId] = useState(null);
  const scrollRef = useRef(null);
  const inputRef = useRef(null);

  useEffect(() => {
    const off = window.aegis?.onTerminalData(({ kind, data }) => {
      if (kind === 'exit') {
        setBusyExecId(null);
        if (data !== '0') {
          setLines((prev) => [...prev, { kind: 'exit', text: `[exit ${data}]` }]);
        }
      } else {
        setLines((prev) => [...prev, { kind, text: data }]);
      }
    });
    return () => off?.();
  }, []);

  useEffect(() => {
    if (visible) inputRef.current?.focus();
  }, [visible]);

  useEffect(() => {
    scrollRef.current?.scrollTo(0, scrollRef.current.scrollHeight);
  }, [lines]);

  if (!visible) return null;

  const run = () => {
    const command = input.trim();
    if (!command || busyExecId !== null) return;
    const execId = nextExecId++;
    setLines((prev) => [...prev, { kind: 'cmd', text: `$ ${command}` }]);
    setInput('');
    setBusyExecId(execId);
    window.aegis?.execCommand(execId, command);
  };

  const onKeyDown = (e) => {
    if (e.key === 'Enter') run();
    if (e.key === 'Escape') onRequestClose();
    if (e.key === 'c' && e.ctrlKey && busyExecId !== null) {
      window.aegis?.killCommand(busyExecId);
    }
  };

  return (
    <div style={styles.overlay}>
      <div style={styles.header}>aegis · bash</div>
      <div style={styles.scrollback} ref={scrollRef}>
        {lines.map((line, i) => (
          <div key={i} style={{ color: lineColor[line.kind] ?? '#d8f6ff' }}>
            {line.text}
          </div>
        ))}
      </div>
      <div style={styles.inputRow}>
        <span style={styles.prompt}>{busyExecId !== null ? '…' : '$'}</span>
        <input
          ref={inputRef}
          style={styles.input}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={onKeyDown}
          spellCheck={false}
          autoComplete="off"
        />
      </div>
    </div>
  );
}
