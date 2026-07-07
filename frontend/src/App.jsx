import React, { useEffect, useRef, useState } from 'react';
import { AegisSphere } from './sphere/AegisSphere.js';
import { createAudioSource } from './audio.js';
import Terminal from './Terminal.jsx';

const styles = {
  root: {
    width: '100%',
    height: '100%',
    position: 'relative',
    background: 'radial-gradient(ellipse at center, #030815 0%, #000005 70%)',
    fontFamily: "'Segoe UI', system-ui, sans-serif",
  },
  canvasWrap: { position: 'absolute', inset: 0 },
  hud: {
    position: 'absolute',
    bottom: '4vh',
    width: '100%',
    textAlign: 'center',
    pointerEvents: 'none',
    letterSpacing: '0.35em',
    textTransform: 'uppercase',
    fontSize: '13px',
  },
};

function hudColor({ linked, sleeping }) {
  if (!linked) return '#ff8c26';
  return sleeping ? '#3d4f8f' : '#33d6ff';
}

function hudText({ linked, sleeping, gameName }) {
  if (!linked) return 'agent link lost — reconnecting';
  if (sleeping) return `sleep mode · ${gameName ?? 'game'} running`;
  return 'aegis online';
}

export default function App() {
  const canvasRef = useRef(null);
  const sphereRef = useRef(null);
  const [linked, setLinked] = useState(false);
  const [sleeping, setSleeping] = useState(false);
  const [gameName, setGameName] = useState(null);
  const [terminalOpen, setTerminalOpen] = useState(false);

  useEffect(() => {
    // WebGL is a nice-to-have, not load-bearing: if context creation fails
    // (no GPU, SwiftShader unavailable), run without the sphere instead of
    // letting the uncaught constructor error unmount the whole tree — that
    // failure mode rendered the entire HUD black on GPU-less targets.
    let sphere = null;
    try {
      sphere = new AegisSphere(canvasRef.current);
    } catch (err) {
      console.warn('[aegis] sphere disabled, WebGL unavailable:', err?.message ?? err);
    }
    sphereRef.current = sphere;

    let audioTimer;
    createAudioSource().then((source) => {
      audioTimer = setInterval(() => sphere?.setAudioLevel(source.getLevel()), 33);
    });

    // In a plain browser (vite preview, no Electron preload) window.aegis is
    // undefined; render the idle sphere rather than crashing, since that path is
    // useful for pure-visual iteration.
    const offEvent = window.aegis?.onEvent((event) => {
      if (event.type === 'Hello') {
        setSleeping(event.sleeping);
      } else if (event.type === 'SleepModeInitiated') {
        setSleeping(true);
        setGameName(event.game_name ?? null);
      } else if (event.type === 'SleepModeEnded') {
        setSleeping(false);
        setGameName(null);
      }
    });
    const offLink = window.aegis?.onLink(({ connected }) => setLinked(connected));

    // Terminal toggle: Electron's global Ctrl+Alt+T lands here, and the same
    // chord is caught locally as a fallback when another app owns the global
    // hook. Voice-open will fire this same toggle once local STT lands.
    const offToggle = window.aegis?.onToggleTerminal(() => setTerminalOpen((v) => !v));
    const onKeyDown = (e) => {
      if (e.ctrlKey && e.altKey && e.key.toLowerCase() === 't') {
        setTerminalOpen((v) => !v);
      }
    };
    window.addEventListener('keydown', onKeyDown);

    return () => {
      clearInterval(audioTimer);
      offEvent?.();
      offLink?.();
      offToggle?.();
      window.removeEventListener('keydown', onKeyDown);
      sphere?.dispose();
    };
  }, []);

  useEffect(() => {
    sphereRef.current?.setSleeping(sleeping);
  }, [sleeping]);

  useEffect(() => {
    sphereRef.current?.setLinkConnected(linked);
  }, [linked]);

  const state = { linked, sleeping, gameName };
  return (
    <div style={styles.root}>
      <div style={styles.canvasWrap}>
        <canvas ref={canvasRef} />
      </div>
      <div style={{ ...styles.hud, color: hudColor(state), textShadow: `0 0 12px ${hudColor(state)}` }}>
        {hudText(state)}
      </div>
      <Terminal visible={terminalOpen} onRequestClose={() => setTerminalOpen(false)} />
    </div>
  );
}
