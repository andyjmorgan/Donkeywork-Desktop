import { useEffect, useRef, useState } from 'react';
import { ConsoleInput, consoleButton, keyboardHid, type InputState } from './consoleInput';
import { consoleSourcePoint } from './consoleGeometry';
import { applyStalledStatus, canOfferConsole, restartForInitialVideoTimeout, type CaptureState } from './consoleRecovery';
import './console.css';
import {DeviceContext} from './DeviceContext';
import {brokerBase, brokerHome} from './brokerRoute';

type State = 'connecting' | 'live' | 'recovering' | 'failed' | 'ended';
type Source = { width: number; height: number };
type CaptureStatus = { state: CaptureState | 'session_ended' | 'session_starting'; source: Source; canResize?: boolean; kind?: string };
function byteRate(value: number) {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)} MB/s`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)} KB/s`;
  return `${Math.round(value)} B/s`;
}

export function App() {
  const managed = new URLSearchParams(window.location.search).has('managed');
  const desktopId = new URLSearchParams(window.location.search).get('desktop');
  const api = (action: string) => desktopId ? `${brokerBase}/desktops/${desktopId}/${action}` : `/api/${action}`;
  const [state, setState] = useState<State>('connecting');
  const [message, setMessage] = useState('Connecting to the console…');
  const [source, setSource] = useState<Source | null>(null);
  const [theme, setTheme] = useState<'dark' | 'light'>('dark');
  const [error, setError] = useState('');
  const [frames, setFrames] = useState(0);
  const [rates, setRates] = useState<{ fps: number; bytes: number } | null>(null);
  const [retry, setRetry] = useState(0);
  const [resizing, setResizing] = useState(false);
  const [canResize, setCanResize] = useState(false);
  const [consoleSession, setConsoleSession] = useState(!managed);
  const [inputState, setInputState] = useState<InputState>('idle');
  const [inputMessage, setInputMessage] = useState('Click the video to control · physical keyboard layout');
  const video = useRef<HTMLVideoElement>(null);
  const surface = useRef<HTMLDivElement>(null);
  const input = useRef<ConsoleInput | null>(null);
  const inputReset = useRef<() => void>(() => {});
  const live = useRef(false);

  useEffect(() => { document.documentElement.dataset.theme = theme; }, [theme]);
  useEffect(() => {
    const element = video.current;
    if (!element) return;
    const heldKeys = new Set<number>();
    const heldButtons = new Set<number>();
    let capturedPointer: number | null = null;
    const abort = new AbortController();
    const signal = abort.signal;
    function clearLocalState() {
      heldKeys.clear(); heldButtons.clear();
      const pointer = capturedPointer; capturedPointer = null;
      if (pointer !== null && element!.hasPointerCapture(pointer)) element!.releasePointerCapture(pointer);
    }
    inputReset.current = clearLocalState;
    function release() { clearLocalState(); input.current?.release(); }
    function active() { return live.current && document.visibilityState === 'visible' && document.hasFocus() && document.activeElement === element; }
    function acquire() { if (active()) input.current?.acquire(); }
    function point(event: { clientX: number; clientY: number }, clamp = false) {
      const raster = input.current?.raster;
      if (!raster || !element!.videoWidth || !element!.videoHeight) return null;
      // Current prototype encodes the complete source raster without downsampling.
      // Reject a changed display until a fresh connection supplies matching topology.
      if (element!.videoWidth !== raster.width || element!.videoHeight !== raster.height) {
        release(); setError('Display dimensions changed; input released until video reconnects.'); return null;
      }
      return consoleSourcePoint(event.clientX, event.clientY, element!.getBoundingClientRect(), raster, clamp ? 'clamp' : 'reject');
    }
    element.addEventListener('focus', acquire, { signal });
    element.addEventListener('blur', release, { signal });
    window.addEventListener('blur', release, { signal });
    window.addEventListener('pagehide', release, { signal });
    document.addEventListener('visibilitychange', () => { if (document.visibilityState !== 'visible') release(); }, { signal });
    element.addEventListener('pointerdown', event => {
      if (!live.current) return;
      if (event.pointerType !== 'mouse') { setError('Touch/pen input is not supported in this alpha.'); return; }
      const button = consoleButton(event.button);
      if (button === null) { event.preventDefault(); setError('Only left, right and middle buttons are supported.'); return; }
      const wasControlling = input.current?.state === 'controlling';
      element.focus({ preventScroll: true });
      acquire();
      event.preventDefault();
      if (!wasControlling || !active()) return; // The acquiring click never reaches the host.
      const position = point(event);
      if (!position) return;
      if (!input.current?.enqueue({ type: 'button', button, down: true, ...position })) return;
      heldButtons.add(button);
      capturedPointer = event.pointerId;
      try { element.setPointerCapture(event.pointerId); }
      catch { release(); }
    }, { signal });
    element.addEventListener('pointermove', event => {
      if (!active() || input.current?.state !== 'controlling' || event.pointerType !== 'mouse') return;
      const position = point(event, heldButtons.size > 0);
      if (position) input.current.enqueue({ type: 'move', ...position }, heldButtons.size === 0);
    }, { signal });
    element.addEventListener('pointerup', event => {
      const button = consoleButton(event.button);
      if (button === null || !heldButtons.has(button)) return;
      event.preventDefault();
      const position = point(event, true);
      if (!position || !active()) { release(); return; }
      input.current?.enqueue({ type: 'button', button, down: false, ...position });
      heldButtons.delete(button);
      if (!heldButtons.size) {
        capturedPointer = null;
        if (element.hasPointerCapture(event.pointerId)) element.releasePointerCapture(event.pointerId);
      }
    }, { signal });
    element.addEventListener('pointercancel', release, { signal });
    element.addEventListener('lostpointercapture', () => { if (heldButtons.size) release(); }, { signal });
    element.addEventListener('contextmenu', event => { if (document.activeElement === element) event.preventDefault(); }, { signal });
    element.addEventListener('auxclick', event => { if (document.activeElement === element) event.preventDefault(); }, { signal });
    element.addEventListener('wheel', event => {
      if (input.current?.state !== 'controlling') return;
      const position = point(event, heldButtons.size > 0);
      if (!position) return;
      event.preventDefault();
      // Convert pixel/line/page deltas to bounded wheel ticks. Preserve both
      // axes; the daemon emits Linux REL_WHEEL/REL_HWHEEL events.
      const scale = event.deltaMode === 1 ? 3 : event.deltaMode === 2 ? 12 : 1 / 100;
      const vertical = Math.max(-32, Math.min(32, Math.round(event.deltaY * scale)));
      const horizontal = Math.max(-32, Math.min(32, Math.round(event.deltaX * scale)));
      if (vertical || horizontal) input.current.enqueue({ type: 'wheel', vertical, horizontal, ...position });
    }, { signal, passive: false });
    element.addEventListener('keydown', event => {
      if (!active() || input.current?.state !== 'controlling') return;
      if (event.code === 'Escape' && event.ctrlKey && event.altKey) { event.preventDefault(); release(); element.blur(); return; }
      event.preventDefault();
      if (event.isComposing || event.key === 'Process') { release(); setError('IME text input is unsupported; use the remote keyboard layout.'); return; }
      if (event.metaKey || event.code === 'MetaLeft' || event.code === 'MetaRight' || event.getModifierState('AltGraph')) {
        release(); setError('Meta/AltGraph input is not yet validated in this browser alpha; control released.'); return;
      }
      const hid = keyboardHid(event.code);
      if (hid === null) { setError('This physical key is not supported by the alpha.'); return; }
      if (event.repeat || heldKeys.has(hid)) return; // Host input stack owns repeat.
      if (input.current.enqueue({ type: 'key', hid, down: true })) heldKeys.add(hid);
    }, { signal });
    element.addEventListener('keyup', event => {
      if (!active() || input.current?.state !== 'controlling') return;
      event.preventDefault();
      const hid = keyboardHid(event.code);
      if (hid !== null && heldKeys.delete(hid)) input.current.enqueue({ type: 'key', hid, down: false });
    }, { signal });
    const tick = window.setInterval(() => input.current?.tick(active()), 50);
    return () => { abort.abort(); window.clearInterval(tick); release(); inputReset.current = () => {}; };
  }, []);
  useEffect(() => {
    let disposed = false;
    let pc: RTCPeerConnection | null = null;
    let poll: number | undefined;
    let deadline: number | undefined;
    let reconnect: number | undefined;
    let lastFrames = 0;
    let staleTicks = 0;
    let statsPending = false;
    let statusRequest: Promise<CaptureStatus> | null = null;
    let previous: { id: string; timestamp: number; frames: number; bytes: number } | undefined;
    const abort = new AbortController();
    function readStatus(): Promise<CaptureStatus> {
      if (statusRequest) return statusRequest;
      const requestAbort = new AbortController();
      const cancel = () => requestAbort.abort();
      abort.signal.addEventListener('abort', cancel, { once: true });
      if (abort.signal.aborted) cancel();
      const timeout = window.setTimeout(cancel, 3000);
      statusRequest = (async () => {
        try {
          const response = await fetch(api('status'), { signal: requestAbort.signal, cache: 'no-store' });
          if (!response.ok) throw new Error('The console bridge is unavailable.');
          const status = await response.json();
          if (managed && status?.state === 'session_ended') return status as CaptureStatus;
          if (managed && status?.state === 'session_starting') throw new Error('Desktop is starting.');
          if (!canOfferConsole(status?.state) || !Number.isSafeInteger(status?.source?.width)
              || !Number.isSafeInteger(status?.source?.height) || status.source.width <= 0 || status.source.height <= 0) {
            throw new Error('Capture status is unavailable or invalid.');
          }
          return status as CaptureStatus;
        } finally {
          window.clearTimeout(timeout); abort.signal.removeEventListener('abort', cancel);
          statusRequest = null;
        }
      })();
      return statusRequest;
    }
    function waiting(reason = 'Waiting for console output…') {
      if (disposed) return;
      live.current = false;
      input.current?.release();
      inputReset.current();
      setState('recovering'); setMessage(reason);
    }
    async function checkInitialDeadline() {
      if (disposed || lastFrames > 0) return;
      try {
        const status = await readStatus();
        if (disposed || lastFrames > 0) return;
        if (status.state === 'session_ended') { setState('ended'); setMessage('Desktop session ended.'); return; }
        if (status.state === 'session_starting') return;
        if (restartForInitialVideoTimeout(status.state, lastFrames)) {
          fail('No video arrived within 20 seconds. Check the capture feed and WebRTC UDP path.');
        } else {
          waiting();
          deadline = window.setTimeout(() => { void checkInitialDeadline(); }, 20_000);
        }
      } catch {
        if (!disposed && lastFrames === 0) fail('No initial video and capture status could not be checked.');
      }
    }
    function checkStalledStatus() {
      const requestFrames = lastFrames;
      waiting();
      void readStatus().then(status => {
        if (disposed || !applyStalledStatus(requestFrames, lastFrames, staleTicks)) return;
        waiting(status.state === 'recovering' ? 'Waiting for console output…' : 'Waiting for video frames…');
      }).catch(() => {
        if (!disposed && applyStalledStatus(requestFrames, lastFrames, staleTicks)) {
          waiting('Waiting for console output… Capture status is temporarily unavailable.');
        }
      });
    }
    function fail(reason: string) {
      if (disposed) return;
      live.current = false; input.current?.fail(); input.current = null;
      setState('failed'); setMessage(`${reason} Retrying automatically…`);
      setRates(null);
      disposed = true; abort.abort(); window.clearInterval(poll); window.clearTimeout(deadline);
      pc?.close(); if (video.current) video.current.srcObject = null;
      reconnect = window.setTimeout(() => setRetry(value => value + 1), 2000);
    }
    async function connect() {
      live.current = false;
      setState('connecting'); setMessage('Connecting to the console…'); setSource(null); setFrames(0); setRates(null); setError('');
      deadline = window.setTimeout(() => { void checkInitialDeadline(); }, 20_000);
      try {
        const status = await readStatus();
        if (disposed) return;
        if (status.state === 'session_ended') {
          window.clearTimeout(deadline); setState('ended'); setMessage('Desktop session ended.');
          setInputState('idle'); return;
        }
        if (status.state === 'recovering') waiting();
        setSource(status.source);
        setCanResize(status.canResize !== false);
        setConsoleSession(status.kind === 'console' || !managed);
        pc = new RTCPeerConnection({ iceServers: [] });
        const channel = pc.createDataChannel('dwconsole.input', { ordered: true });
        const controller = new ConsoleInput(channel, status.source, (next, notice) => {
          if (disposed) return;
          setInputState(next); setInputMessage(notice);
          if (next !== 'controlling') inputReset.current();
        });
        input.current = controller;
        setInputState(controller.state);
        setInputMessage('Click the video to control · physical keyboard layout');
        channel.onmessage = event => controller.receive(event.data);
        channel.onclose = () => {
          if (disposed) return;
          if (managed) { fail('Input connection ended; reconnecting the managed viewer.'); return; }
          if (controller.state !== 'unavailable') controller.fail();
        };
        channel.onerror = () => controller.fail('Input transport error. Video remains view only.');
        const receiver = pc.addTransceiver('video', { direction: 'recvonly' });
        const codecs = RTCRtpReceiver.getCapabilities('video')?.codecs.filter(codec => codec.mimeType.toLowerCase() === 'video/h264');
        if (!codecs?.length) throw new Error('This browser has no H.264 WebRTC decoder.');
        receiver.setCodecPreferences(codecs);
        pc.ontrack = event => {
          if (!disposed && video.current) {
            video.current.srcObject = event.streams[0] ?? new MediaStream([event.track]);
            void video.current.play().catch(() => fail('Browser playback was blocked.'));
          }
        };
        pc.onconnectionstatechange = () => {
          if (pc && ['failed', 'disconnected', 'closed'].includes(pc.connectionState)) fail('Console connection ended.');
        };
        await pc.setLocalDescription(await pc.createOffer());
        await new Promise<void>((resolve, reject) => {
          if (pc!.iceGatheringState === 'complete') { resolve(); return; }
          const changed = () => { if (pc!.iceGatheringState === 'complete') { pc!.removeEventListener('icegatheringstatechange', changed); resolve(); } };
          pc!.addEventListener('icegatheringstatechange', changed);
          abort.signal.addEventListener('abort', () => { pc?.removeEventListener('icegatheringstatechange', changed); reject(new Error('Connection cancelled')); }, { once: true });
        });
        if (disposed) return;
        const response = await fetch(api('offer'), { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(pc.localDescription), signal: abort.signal });
        if (!response.ok) throw new Error(`Console negotiation failed (${response.status}).`);
        await pc.setRemoteDescription(await response.json());
        poll = window.setInterval(() => {
          if (!pc || disposed || statsPending) return;
          statsPending = true;
          void pc.getStats().then(stats => {
            if (disposed) return;
            let advanced = false;
            stats.forEach(report => {
              if (report.type !== 'inbound-rtp' || report.kind !== 'video') return;
              const decoded = Number(report.framesDecoded ?? 0);
              const bytes = Number(report.bytesReceived ?? 0);
              const timestamp = Number(report.timestamp);
              if (previous && previous.id === report.id && timestamp > previous.timestamp
                  && decoded >= previous.frames && bytes >= previous.bytes) {
                const seconds = (timestamp - previous.timestamp) / 1000;
                setRates({ fps: (decoded - previous.frames) / seconds, bytes: (bytes - previous.bytes) / seconds });
              } else { setRates(null); }
              previous = { id: report.id, timestamp, frames: decoded, bytes };
              if (decoded > lastFrames) {
                advanced = true;
                live.current = true;
                staleTicks = 0; window.clearTimeout(deadline); setState('live');
                setMessage(status.kind === 'console' ? 'Existing console' : managed ? 'Managed desktop' : 'Physical console'); setFrames(decoded); lastFrames = decoded;
              }
            });
            // No inbound report at all also counts as waiting for first video.
            if (!advanced && ++staleTicks >= 2) checkStalledStatus();
          }).catch(() => fail('Unable to read the video connection.')).finally(() => { statsPending = false; });
        }, 1000);
      } catch (cause) { if (!disposed) fail(cause instanceof Error ? cause.message : 'Connection failed.'); }
    }
    void connect();
    return () => {
      live.current = false; input.current?.fail(); input.current = null;
      disposed = true; abort.abort(); window.clearInterval(poll); window.clearTimeout(deadline);
      window.clearTimeout(reconnect);
      pc?.close(); if (video.current) video.current.srcObject = null;
    };
  }, [retry]);

  async function fullscreen() {
    try { if (document.fullscreenElement) await document.exitFullscreen(); else await surface.current?.requestFullscreen(); }
    catch { setError('Fullscreen was not permitted by the browser.'); }
  }
  async function resize(width: number, height: number) {
    setResizing(true); setError('');
    input.current?.release();
    try {
      for (let attempt = 0; attempt < 50 && input.current?.state === 'releasing'; attempt++)
        await new Promise(resolve => setTimeout(resolve, 20));
      const response = await fetch(api('resize'), { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ width, height }) });
      if (!response.ok || (await response.json()).status !== 'applied') throw new Error('Resize was not applied. Release input and retry.');
    } catch (error) { setError(error instanceof Error ? error.message : 'Resize request failed; check the displayed dimensions.'); }
    finally { setResizing(false); }
  }
  async function startDesktop() {
    if (desktopId) { window.location.href = brokerHome; return; }
    setResizing(true); setError('');
    try {
      const response = await fetch('/api/session/connect', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{}' });
      if (!response.ok) throw new Error('Desktop could not be started.');
      setRetry(value => value + 1);
    } catch (error) { setError(error instanceof Error ? error.message : 'Desktop could not be started.'); }
    finally { setResizing(false); }
  }
  return <div className="console-shell">
    <header className="console-header">
      <a className="brand" href="/"><img src="/donkeywork.png" alt="" /><span>DonkeyWork<small>DESKTOP</small></span></a>
      <span className="console-label">{consoleSession ? 'Existing console' : 'Managed session'}</span>
      {managed && <a href={brokerHome}>All desktops</a>}
      <button aria-label={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`} onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}>{theme === 'dark' ? 'Light' : 'Dark'}</button>
    </header>
    <main className="console-main">
      <DeviceContext/>
      <div className="console-heading"><div><span className="eyebrow accent">{consoleSession ? 'EXISTING CONSOLE' : 'MANAGED DESKTOP'}</span><h1>Your desktop, here.</h1></div>
        <div className="console-actions"><button onClick={() => input.current?.release()} disabled={inputState !== 'controlling'}>Release input</button><button onClick={fullscreen} disabled={state !== 'live'}>Fullscreen</button>
          {managed && canResize && <><button disabled={resizing || state !== 'live' || source?.width === 1920} onClick={() => resize(1920, 1080)}>1080p</button><button disabled={resizing || state !== 'live' || source?.width === 3840} onClick={() => resize(3840, 2160)}>4K</button></>}
        </div>
      </div>
      <section aria-label="Console video" className="console-view" ref={surface} data-input-state={inputState}>
        <video ref={video} autoPlay playsInline muted tabIndex={0} aria-label="Live physical console; focus to request keyboard and mouse control" />
        {state !== 'live' && !(state === 'recovering' && frames > 0) && <div className="console-placeholder"><span className="eyebrow">{state === 'ended' ? 'SESSION ENDED' : state === 'recovering' ? 'WAITING FOR CONSOLE' : state === 'connecting' ? 'CONNECTING' : 'RECONNECTING'}</span><p>{message}</p>{state === 'ended' && <><p>This desktop has ended. Choose another desktop or create one.</p><button disabled={resizing} onClick={startDesktop}>Back to desktops</button><p>No new session will be created automatically.</p></>}</div>}
      </section>
      {inputState === 'unavailable' && <p role="status">{inputMessage}</p>}
      <footer className="console-status" role="status"><span><i className={`dot ${state === 'live' ? 'teal' : 'muted'}`} />{message}</span><span title="Decoded video frames per second and received RTP video payload bytes per second; excludes network headers." data-testid="stream-metrics">{source ? `${source.width} × ${source.height} · H.264` : 'H.264 video'}{rates ? ` · ${rates.fps.toFixed(1)} FPS · ${byteRate(rates.bytes)}` : ' · — FPS · — B/s'}</span></footer>
      {error && <p role="alert">{error}</p>}
    </main>
  </div>;
}
