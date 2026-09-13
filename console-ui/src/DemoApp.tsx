import { useEffect, useReducer, useRef, useState } from 'react';
import { demoModes, demoReducer, displayPoint, initialState, label, layout, sameMode, type ScaleMode } from './model';

function Icon({ name }: { name: 'screen' | 'terminal' | 'expand' | 'arrow' | 'lock' | 'plus' | 'sun' | 'moon' | 'menu' | 'close' }) {
  const paths = {
    screen: <><rect x="3" y="4" width="18" height="13" rx="2" /><path d="M8 21h8M12 17v4" /></>,
    terminal: <><path d="m5 6 5 6-5 6M13 18h6" /></>,
    expand: <><path d="M8 3H3v5M16 3h5v5M21 16v5h-5M8 21H3v-5" /></>,
    arrow: <><path d="M5 12h14m-5-5 5 5-5 5" /></>,
    lock: <><rect x="5" y="10" width="14" height="11" rx="2" /><path d="M8 10V7a4 4 0 0 1 8 0v3M12 14v3" /></>,
    plus: <><path d="M12 5v14M5 12h14" /></>,
    sun: <><circle cx="12" cy="12" r="4" /><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1 1m12 12 1 1M5 19l1-1M18 6l1-1" /></>,
    moon: <path d="M20 15A9 9 0 0 1 9 4a9 9 0 1 0 11 11Z" />,
    menu: <path d="M4 6h16M4 12h16M4 18h16" />,
    close: <path d="m6 6 12 12M18 6 6 18" />,
  };
  return <svg className="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>;
}

export function DemoApp() {
  const [state, dispatch] = useReducer(demoReducer, initialState);
  const [scaleMode, setScaleMode] = useState<ScaleMode>('fit');
  const [selectedMode, setSelectedMode] = useState(0);
  const [simulateFailure, setSimulateFailure] = useState(false);
  const [viewport, setViewport] = useState({ width: 960, height: 540 });
  const [point, setPoint] = useState<{ x: number; y: number } | null>(null);
  const [fullscreen, setFullscreen] = useState(false);
  const [fullscreenError, setFullscreenError] = useState('');
  const [navigationOpen, setNavigationOpen] = useState(false);
  const [theme, setTheme] = useState<'dark' | 'light'>(() => {
    try { return localStorage.getItem('dwdesktop-demo-theme') === 'light' ? 'light' : 'dark'; }
    catch { return 'dark'; }
  });
  const workspace = useRef<HTMLElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const previewButton = useRef<HTMLButtonElement>(null);
  const navigation = useRef<HTMLElement>(null);
  const navigationButton = useRef<HTMLButtonElement>(null);
  const closeNavigationButton = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem('dwdesktop-demo-theme', theme); } catch { /* Preference persistence is optional. */ }
  }, [theme]);
  useEffect(() => {
    if (!navigationOpen) return;
    closeNavigationButton.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setNavigationOpen(false);
        navigationButton.current?.focus();
      }
      if (event.key === 'Tab') {
        const targets = navigation.current?.querySelectorAll<HTMLElement>('a[href], button:not([disabled])');
        if (!targets?.length) return;
        const first = targets[0];
        const last = targets[targets.length - 1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [navigationOpen]);

  useEffect(() => {
    if (!state.pending) return;
    const timer = window.setTimeout(() => dispatch({ type: simulateFailure ? 'resize-failed' : 'resize-applied' }), 550);
    return () => window.clearTimeout(timer);
  }, [state.pending, simulateFailure]);
  useEffect(() => {
    setPoint(null);
    const index = demoModes.findIndex(mode => sameMode(mode, state.mode));
    setSelectedMode(index);
  }, [state.mode]);
  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return;
    const observer = new ResizeObserver(entries => {
      const { width, height } = entries[0].contentRect;
      setViewport({ width: Math.max(1, width - 32), height: Math.max(1, height - 32) });
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [state.open]);
  useEffect(() => {
    const onChange = () => setFullscreen(document.fullscreenElement === workspace.current);
    document.addEventListener('fullscreenchange', onChange);
    return () => document.removeEventListener('fullscreenchange', onChange);
  }, []);

  const render = layout(state.mode, viewport, scaleMode);
  async function toggleFullscreen() {
    setFullscreenError('');
    try {
      if (document.fullscreenElement) await document.exitFullscreen();
      else if (workspace.current?.requestFullscreen) await workspace.current.requestFullscreen();
      else setFullscreenError('Fullscreen is unavailable in this browser.');
    } catch { setFullscreenError('The browser did not allow fullscreen. The preview remains available.'); }
  }
  function closePreview() {
    dispatch({ type: 'close' });
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
    previewButton.current?.focus();
  }

  return <div className="app-shell">
    <a className="skip-link" href="#workspace">Skip to desktop workspace</a>
    {navigationOpen && <button className="navigation-overlay" aria-label="Close device navigation" onClick={() => setNavigationOpen(false)} />}
    <aside ref={navigation} id="device-navigation" className={`sidebar ${navigationOpen ? 'navigation-open' : ''}`} aria-label="Device navigation">
      <div className="brand-row"><a className="brand" href="#"><img src="/donkeywork.png" alt="" width="32" height="32" /><span>DonkeyWork<small>DESKTOP</small></span></a><button ref={closeNavigationButton} className="icon-button mobile-only" aria-label="Close navigation" onClick={() => { setNavigationOpen(false); navigationButton.current?.focus(); }}><Icon name="close" /></button></div>
      <div className="sidebar-group"><span className="eyebrow">WORKSPACE</span><div className="nav-current"><Icon name="screen" />Devices<span className="count">1</span></div></div>
      <div className="device-heading"><span className="eyebrow">YOUR DEVICES</span><span className="fixture-label">DEMO</span></div>
      <button className="device-selected" onClick={() => { dispatch({ type: 'open' }); setNavigationOpen(false); }} aria-label="Open Spark local preview"><span className="device-glyph"><Icon name="screen" /></span><span>Spark<small><i className="dot muted" />Not connected</small></span><span className="device-chevron">›</span></button>
      <button className="register-button" disabled aria-describedby="register-hint"><Icon name="plus" />Register device</button>
      <p className="small-hint" id="register-hint">Registration becomes available when the broker is connected.</p>
      <div className="sidebar-bottom"><span className="status-tag"><i className="dot amber" />Local demo</span><p>Explore the console layout.<br />No remote sessions are active.</p><span className="version">DONKEYWORK DESKTOP · ALPHA</span></div>
    </aside>

    <div className="main-shell">
      <header className="topbar"><button ref={navigationButton} className="icon-button mobile-only" aria-label="Open device navigation" aria-expanded={navigationOpen} aria-controls="device-navigation" onClick={() => setNavigationOpen(true)}><Icon name="menu" /></button><div className="breadcrumb">Devices<span>/</span><strong>Spark</strong></div><div className="auth-status"><button className="icon-button theme-toggle" onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')} aria-label={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`} title={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}><Icon name={theme === 'dark' ? 'sun' : 'moon'} /></button><Icon name="lock" /><span>Keycloak not connected</span><button disabled>Sign in</button></div></header>
      <main>
        <div className="page-heading"><div><span className="eyebrow accent">YOUR MACHINES. ONE WORKSPACE.</span><h1>Spark</h1><p>Console access, with room to build forward.</p></div><button className="primary-button" ref={previewButton} onClick={() => { dispatch({ type: 'open' }); window.requestAnimationFrame(() => workspace.current?.focus()); }}>Open local preview<Icon name="arrow" /></button></div>

        <section className="device-summary" aria-label="Spark device summary"><div><span className="detail-label">DEVICE</span><strong>office-spark-1</strong></div><div><span className="detail-label">PLATFORM · FIXTURE</span><strong>Linux <span className="subtle">/ ARM64</span></strong></div><div><span className="detail-label">CONNECTION</span><strong><i className="dot muted" />Not connected</strong></div><div><span className="detail-label">REMOTE SESSION</span><strong className="subtle">None</strong></div></section>

        <section id="workspace" className="workspace" ref={workspace} tabIndex={-1} aria-labelledby="workspace-title">
          <div className="workspace-header"><div className="workspace-tabs"><span className="tab active" id="workspace-title"><Icon name="screen" />Desktop preview</span><button className="tab" disabled title="The browser terminal adapter is not connected"><Icon name="terminal" />Terminal<span className="tiny-label">Soon</span></button></div><span className="preview-badge">SIMULATED DISPLAY</span></div>

          {state.open ? <>
            <div className="viewer-toolbar"><div className="display-name"><i className="dot teal" />Display 1<span className="subtle">· Preview</span></div><div className="view-controls"><div className="segmented" aria-label="Browser display scale"><button aria-pressed={scaleMode === 'fit'} onClick={() => setScaleMode('fit')}>Fit</button><button aria-pressed={scaleMode === 'native'} onClick={() => setScaleMode('native')}>Native 1:1</button></div><button className="icon-button" onClick={() => void toggleFullscreen()} aria-label={fullscreen ? 'Exit fullscreen preview' : 'Fullscreen preview'} title="Fullscreen preview"><Icon name="expand" /></button><button className="text-button" onClick={closePreview}>Close preview</button></div></div>

            <div className="viewport" ref={viewportRef} tabIndex={0} aria-label="Scrollable simulated display" aria-describedby="preview-input-hint">
              <div className="display-centering" style={{ minWidth: render.width + 32, minHeight: render.height + 32 }}>
                <div className={`display-raster ${state.pending ? 'is-changing' : ''}`} style={{ width: render.width, height: render.height }} onClick={event => {
                  if (state.pending) return;
                  setPoint(displayPoint(event.clientX, event.clientY, event.currentTarget.getBoundingClientRect(), state.mode));
                }}>
                  <svg viewBox={`0 0 ${state.mode.width} ${state.mode.height}`} width="100%" height="100%" role="img" aria-label={`Local test pattern at simulated ${label(state.mode)} resolution. This is not a screenshot of Spark.`}>
                    <defs><pattern id="grid" width="80" height="80" patternUnits="userSpaceOnUse"><path d="M 80 0 L 0 0 0 80" fill="none" stroke="#273442" strokeWidth="1" /></pattern><radialGradient id="light"><stop offset="0" stopColor="#23413e" stopOpacity=".65" /><stop offset="1" stopColor="#101c29" stopOpacity="0" /></radialGradient></defs>
                    <rect width="100%" height="100%" fill="#101c29" /><rect width="100%" height="100%" fill="url(#grid)" /><ellipse cx="65%" cy="35%" rx="60%" ry="80%" fill="url(#light)" />
                    <g transform={`translate(${state.mode.width / 2},${state.mode.height / 2})`}>
                      <rect x="-590" y="-235" width="1180" height="470" rx="24" fill="#101923" stroke="#3d5b60" strokeWidth="2" />
                      <text x="-510" y="-130" fill="#79d6bf" fontSize="23" fontFamily="monospace" letterSpacing="4">DONKEYWORK / DISPLAY PREVIEW</text>
                      <text x="-510" y="-25" fill="#e4ecf3" fontSize="66" fontFamily="system-ui, sans-serif" fontWeight="550">A place for your desktop.</text>
                      <text x="-510" y="65" fill="#9fafbf" fontSize="29" fontFamily="system-ui, sans-serif">Local test pattern. No screen capture or remote input.</text>
                      <path d="M-510 125H510" stroke="#2c3d4b" strokeWidth="2" />
                      <text x="-510" y="182" fill="#a4b6c8" fontSize="26" fontFamily="monospace">{label(state.mode)} · SIMULATED</text>
                      <rect x="355" y="154" width="38" height="28" fill="#64c3a7" /><rect x="400" y="154" width="38" height="28" fill="#809dca" /><rect x="445" y="154" width="38" height="28" fill="#cfad72" />
                    </g>
                  </svg>
                  {state.pending && <div className="resize-overlay" role="status">Applying demo mode…</div>}
                </div>
              </div>
            </div>

            <div className="display-footer"><span>{label(state.mode)} <span className="subtle">simulated</span></span><span>Viewer scale <strong>{Math.round(render.scale * 100)}%</strong></span><span className="coordinate-label" aria-live="polite">{point ? `Local point ${point.x}, ${point.y}` : 'Input is not connected'}</span></div>
            <div className="resolution-panel"><div><h2>Remote desktop resolution <span className="tiny-label">DEMO</span></h2><p>This control simulates a display-mode change. Fit and Native only change the browser view.</p></div><div className="resolution-controls"><label className="sr-only" htmlFor="resolution">Supported demo resolution</label><select id="resolution" value={selectedMode} disabled={!!state.pending} onChange={e => setSelectedMode(Number(e.target.value))}>{demoModes.map((mode, index) => <option key={index} value={index}>{label(mode)}{index === 0 ? ' · 4K' : ' · 1080p'}</option>)}</select><button className="secondary-button" disabled={!!state.pending || sameMode(demoModes[selectedMode], state.mode)} onClick={() => dispatch({ type: 'resize', mode: demoModes[selectedMode] })}>Apply demo mode</button></div></div>
          </> : <div className="closed-preview"><Icon name="screen" /><h2>Preview closed</h2><p>Open the local preview to explore display controls.</p><button className="secondary-button" onClick={() => dispatch({ type: 'open' })}>Open preview</button></div>}
          <div className="workspace-notice" role="status">{fullscreenError || state.notice}</div>
        </section>

        <div className="below-workspace"><p id="preview-input-hint">Preview clicks show local coordinates only. Keyboard input is never forwarded.</p><details><summary>Demo behaviour</summary><label className="failure-option"><input type="checkbox" checked={simulateFailure} disabled={!!state.pending} onChange={e => setSimulateFailure(e.target.checked)} />Simulate the next resolution change failing</label><p>Fixtures only. Device presence, Keycloak, streaming and terminal access are not connected.</p></details></div>
      </main>
      <footer className="page-footer"><span>DonkeyWork Desktop</span><span>Local interface preview · No broker requests</span></footer>
    </div>
  </div>;
}
