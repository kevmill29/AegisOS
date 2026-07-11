import React, { useEffect, useRef, useState } from 'react';

// GUI installer — the Windows-switcher path to disk. Only rendered when the
// live-ISO backend exists (installerCapable), so installed systems and dev
// builds never show it. Five human answers (disk, name, password ×2, computer
// name); everything Arch-shaped happens root-side in aegis-install-backend,
// which streams progress back up. The user never sees a TUI, a mirror, or a
// partition table.

const cyan = '#33d6ff';
const dim = 'rgba(216, 246, 255, 0.75)';

const styles = {
  panel: {
    position: 'absolute',
    left: '50%',
    top: '50%',
    transform: 'translate(-50%, -50%)',
    width: 'min(560px, 92vw)',
    maxHeight: '82vh',
    overflowY: 'auto',
    background: 'rgba(2, 10, 24, 0.92)',
    border: `1px solid rgba(51, 214, 255, 0.35)`,
    borderRadius: '10px',
    boxShadow: '0 0 48px rgba(51, 214, 255, 0.18)',
    backdropFilter: 'blur(8px)',
    padding: '28px 32px',
    color: '#d8f6ff',
    fontFamily: "'Segoe UI', system-ui, sans-serif",
    zIndex: 20,
  },
  eyebrow: {
    color: cyan,
    letterSpacing: '0.3em',
    textTransform: 'uppercase',
    fontSize: '10px',
    marginBottom: '10px',
  },
  title: { fontSize: '22px', fontWeight: 300, marginBottom: '8px' },
  body: { fontSize: '14px', color: dim, lineHeight: 1.55, marginBottom: '20px' },
  row: { display: 'flex', gap: '10px', justifyContent: 'flex-end', marginTop: '22px' },
  primary: {
    background: 'rgba(51, 214, 255, 0.16)',
    border: `1px solid ${cyan}`,
    color: cyan,
    borderRadius: '6px',
    padding: '9px 22px',
    fontSize: '13px',
    letterSpacing: '0.08em',
    cursor: 'pointer',
  },
  ghost: {
    background: 'transparent',
    border: '1px solid rgba(216, 246, 255, 0.25)',
    color: dim,
    borderRadius: '6px',
    padding: '9px 18px',
    fontSize: '13px',
    cursor: 'pointer',
  },
  diskCard: (selected) => ({
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'baseline',
    padding: '13px 16px',
    marginBottom: '8px',
    borderRadius: '8px',
    cursor: 'pointer',
    border: selected ? `1px solid ${cyan}` : '1px solid rgba(216, 246, 255, 0.18)',
    background: selected ? 'rgba(51, 214, 255, 0.10)' : 'rgba(216, 246, 255, 0.03)',
  }),
  label: { display: 'block', fontSize: '12px', color: dim, margin: '14px 0 5px' },
  input: {
    width: '100%',
    boxSizing: 'border-box',
    background: 'rgba(216, 246, 255, 0.05)',
    border: '1px solid rgba(216, 246, 255, 0.25)',
    borderRadius: '6px',
    padding: '9px 12px',
    color: '#d8f6ff',
    fontSize: '14px',
    outline: 'none',
  },
  warn: {
    border: '1px solid rgba(255, 154, 107, 0.5)',
    background: 'rgba(255, 154, 107, 0.08)',
    color: '#ffb28a',
    borderRadius: '8px',
    padding: '11px 14px',
    fontSize: '13px',
    lineHeight: 1.5,
  },
  barTrack: {
    height: '6px',
    borderRadius: '3px',
    background: 'rgba(216, 246, 255, 0.10)',
    overflow: 'hidden',
    margin: '18px 0 10px',
  },
  bar: (pct) => ({
    height: '100%',
    width: `${pct}%`,
    background: cyan,
    boxShadow: `0 0 12px ${cyan}`,
    transition: 'width 0.6s ease',
  }),
  log: {
    fontFamily: "'Cascadia Mono', 'Consolas', monospace",
    fontSize: '11px',
    color: 'rgba(216, 246, 255, 0.45)',
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-word',
    maxHeight: '110px',
    overflowY: 'auto',
    marginTop: '12px',
  },
  error: { color: '#ff9a6b', fontSize: '13px', lineHeight: 1.5, marginTop: '14px' },
};

const USERNAME_RE = /^[a-z_][a-z0-9_-]{0,31}$/;
const HOSTNAME_RE = /^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?$/;

export default function Installer({ visible, onRequestClose }) {
  const [step, setStep] = useState('welcome'); // welcome|disks|account|confirm|installing|done|failed
  const [disks, setDisks] = useState(null);
  const [disk, setDisk] = useState(null);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPw, setConfirmPw] = useState('');
  const [hostname, setHostname] = useState('aegis');
  const [pct, setPct] = useState(0);
  const [phaseMsg, setPhaseMsg] = useState('');
  const [logLines, setLogLines] = useState([]);
  const [errorMsg, setErrorMsg] = useState(null);
  const logRef = useRef(null);
  const installing = step === 'installing';

  useEffect(() => {
    if (!visible) return undefined;
    const off = window.aegis?.onInstallerProgress((ev) => {
      if (ev.event === 'phase') {
        setPct(ev.pct);
        setPhaseMsg(ev.msg);
      } else if (ev.event === 'log') {
        setLogLines((prev) => [...prev.slice(-120), ev.line]);
      } else if (ev.event === 'error') {
        setErrorMsg(ev.msg);
        setStep('failed');
      } else if (ev.event === 'done') {
        setStep('done');
      } else if (ev.event === 'exit' && ev.code !== 0) {
        // Backend died without a friendly error event — still surface it.
        setStep((s) => (s === 'installing' ? 'failed' : s));
        setErrorMsg((m) => m ?? 'The installer stopped unexpectedly. Check the details below and try again.');
      }
    });
    return () => off?.();
  }, [visible]);

  useEffect(() => {
    if (visible && step === 'disks' && disks === null) {
      window.aegis?.installerListDisks().then(setDisks);
    }
  }, [visible, step, disks]);

  useEffect(() => {
    logRef.current?.scrollTo(0, logRef.current.scrollHeight);
  }, [logLines]);

  if (!visible) return null;

  const close = () => {
    if (!installing) onRequestClose();
  };

  const accountValid =
    USERNAME_RE.test(username) &&
    password.length >= 4 &&
    password === confirmPw &&
    HOSTNAME_RE.test(hostname);

  const begin = () => {
    setPct(0);
    setPhaseMsg('Starting');
    setLogLines([]);
    setErrorMsg(null);
    setStep('installing');
    window.aegis?.installerStart({ disk: disk.path, username, password, hostname });
  };

  return (
    <div style={styles.panel} onKeyDown={(e) => e.key === 'Escape' && close()}>
      {step === 'welcome' && (
        <>
          <div style={styles.eyebrow}>aegis os · setup</div>
          <div style={styles.title}>Install Aegis OS on this computer</div>
          <div style={styles.body}>
            You&apos;re running Aegis live from the USB stick right now — nothing has been
            changed on this computer. Installing takes about 15 minutes and asks four
            questions. You&apos;ll need an internet connection (a network cable is easiest).
          </div>
          <div style={styles.row}>
            <button style={styles.ghost} onClick={close}>Keep trying it live</button>
            <button style={styles.primary} onClick={() => setStep('disks')}>Get started</button>
          </div>
        </>
      )}

      {step === 'disks' && (
        <>
          <div style={styles.eyebrow}>step 1 of 3 · where</div>
          <div style={styles.title}>Choose a disk</div>
          <div style={styles.body}>
            Aegis will use the whole disk. <b>Everything currently on it will be erased.</b>
          </div>
          {disks === null && <div style={styles.body}>Looking for disks…</div>}
          {disks !== null && disks.length === 0 && (
            <div style={styles.warn}>
              No suitable disks found (Aegis needs a 30&nbsp;GB or larger internal disk).
            </div>
          )}
          {(disks ?? []).map((d) => (
            <div key={d.path} style={styles.diskCard(disk?.path === d.path)} onClick={() => setDisk(d)}>
              <span>{d.model}</span>
              <span style={{ color: dim, fontSize: '13px' }}>{d.size} · {d.path}</span>
            </div>
          ))}
          <div style={styles.row}>
            <button style={styles.ghost} onClick={() => { setDisks(null); }}>Refresh</button>
            <button style={styles.ghost} onClick={close}>Cancel</button>
            <button style={{ ...styles.primary, opacity: disk ? 1 : 0.4 }} disabled={!disk} onClick={() => setStep('account')}>
              Continue
            </button>
          </div>
        </>
      )}

      {step === 'account' && (
        <>
          <div style={styles.eyebrow}>step 2 of 3 · who</div>
          <div style={styles.title}>Create your account</div>
          <label style={styles.label}>Username (lowercase, no spaces)</label>
          <input style={styles.input} value={username} autoFocus spellCheck={false}
            onChange={(e) => setUsername(e.target.value.toLowerCase())} />
          <label style={styles.label}>Password</label>
          <input style={styles.input} type="password" value={password}
            onChange={(e) => setPassword(e.target.value)} />
          <label style={styles.label}>Confirm password</label>
          <input style={styles.input} type="password" value={confirmPw}
            onChange={(e) => setConfirmPw(e.target.value)} />
          {confirmPw && password !== confirmPw && (
            <div style={styles.error}>Passwords don&apos;t match yet.</div>
          )}
          <label style={styles.label}>Computer name</label>
          <input style={styles.input} value={hostname} spellCheck={false}
            onChange={(e) => setHostname(e.target.value)} />
          <div style={styles.row}>
            <button style={styles.ghost} onClick={() => setStep('disks')}>Back</button>
            <button style={{ ...styles.primary, opacity: accountValid ? 1 : 0.4 }} disabled={!accountValid}
              onClick={() => setStep('confirm')}>
              Continue
            </button>
          </div>
        </>
      )}

      {step === 'confirm' && (
        <>
          <div style={styles.eyebrow}>step 3 of 3 · confirm</div>
          <div style={styles.title}>Ready to install</div>
          <div style={styles.body}>
            Aegis OS for <b>{username}</b> on <b>{hostname}</b>, using{' '}
            <b>{disk.model} ({disk.size})</b>.
          </div>
          <div style={styles.warn}>
            Last chance: everything on {disk.model} ({disk.path}) will be permanently erased.
          </div>
          <div style={styles.row}>
            <button style={styles.ghost} onClick={() => setStep('account')}>Back</button>
            <button style={styles.primary} onClick={begin}>Erase disk &amp; install</button>
          </div>
        </>
      )}

      {installing && (
        <>
          <div style={styles.eyebrow}>installing</div>
          <div style={styles.title}>{phaseMsg || 'Working…'}</div>
          <div style={styles.barTrack}><div style={styles.bar(pct)} /></div>
          <div style={styles.body}>
            Leave the computer on — this takes a while, mostly downloading. The sphere
            keeps you company.
          </div>
          <div style={styles.log} ref={logRef}>{logLines.join('\n')}</div>
        </>
      )}

      {step === 'done' && (
        <>
          <div style={styles.eyebrow}>complete</div>
          <div style={styles.title}>Aegis OS is installed</div>
          <div style={styles.body}>
            Remove the USB stick, then restart. The computer will boot straight into
            your own Aegis sphere.
          </div>
          <div style={styles.row}>
            <button style={styles.primary} onClick={() => window.aegis?.installerReboot()}>
              Restart now
            </button>
          </div>
        </>
      )}

      {step === 'failed' && (
        <>
          <div style={styles.eyebrow}>problem</div>
          <div style={styles.title}>The install hit a snag</div>
          <div style={styles.error}>{errorMsg}</div>
          <div style={styles.log} ref={logRef}>{logLines.slice(-30).join('\n')}</div>
          <div style={styles.row}>
            <button style={styles.ghost} onClick={close}>Close</button>
            <button style={styles.primary} onClick={() => { setDisks(null); setStep('disks'); }}>
              Try again
            </button>
          </div>
        </>
      )}
    </div>
  );
}
