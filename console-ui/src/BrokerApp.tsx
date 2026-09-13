import { useEffect, useState } from 'react';
import './console.css';
import './broker.css';
import {DeviceContext} from './DeviceContext';
import {brokerBase, brokerHome, desktopURL} from './brokerRoute';

type Desktop = { id: string; user: string; environment: string; state: string; createdAt: number; kind?: 'console' | 'managed' };
type Environment = { id: string; name: string; validation: string };

function requestId() {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 15) | 64; bytes[8] = (bytes[8] & 63) | 128;
  const hex = Array.from(bytes, n => n.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0,8)}-${hex.slice(8,12)}-${hex.slice(12,16)}-${hex.slice(16,20)}-${hex.slice(20)}`;
}

export function BrokerApp() {
	const [sessionKind, setSessionKind] = useState(new URLSearchParams(location.search).get('view') === 'terminal' ? 'terminal' : 'console');
  const [desktops, setDesktops] = useState<Desktop[]>([]);
  const [environments, setEnvironments] = useState<Environment[]>([]);
  const [history, setHistory] = useState<Desktop[]>([]);
  const [environment, setEnvironment] = useState('gnome');
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [theme, setTheme] = useState<'dark' | 'light'>('dark');
  useEffect(() => { document.documentElement.dataset.theme = theme; }, [theme]);
  async function refresh() {
    const [inventory, available] = await Promise.all([fetch(`${brokerBase}/desktops`), fetch(`${brokerBase}/environments`)]);
    if (!inventory.ok || !available.ok) throw new Error('The desktop broker is unavailable.');
    const list = await inventory.json(), profiles = await available.json();
    if (!Array.isArray(list.desktops) || !Array.isArray(profiles.environments)) throw new Error('Invalid broker response.');
    setDesktops(list.desktops); setHistory(list.history ?? []); setEnvironments(profiles.environments); setLoaded(true);
    setEnvironment(current => profiles.environments.some((p: Environment) => p.id === current) ? current : profiles.environments[0]?.id ?? '');
  }
  useEffect(() => {
    let stopped = false;
    const update = () => { if (!stopped) void refresh().catch(e => setError(e.message)); };
    update(); const interval = window.setInterval(update, 2000);
    return () => { stopped = true; clearInterval(interval); };
  }, []);
  async function create() {
    setBusy(true); setError(''); setNotice('');
    try {
      const response = await fetch(`${brokerBase}/desktops`, { method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ environment, requestId: requestId() }) });
      if (!response.ok) throw new Error('Desktop creation was rejected. Check the available environment and session limit.');
      setNotice('Desktop is starting. Reconnect when it is ready.'); await refresh();
    } catch (e) { setError(e instanceof Error ? e.message : 'Creation failed.'); }
    finally { setBusy(false); }
  }
  async function reconnect(id: string) {
    setError('');
    const response = await fetch(`${brokerBase}/desktops/${id}/reconnect`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{}' });
    if (!response.ok) { setError('That desktop is no longer ready. Refreshing the list.'); await refresh(); return; }
    window.location.href = desktopURL(id, sessionKind);
  }
  async function close(id: string) {
    if (!window.confirm('End this desktop and its running applications?')) return;
    const response = await fetch(`${brokerBase}/desktops/${id}/close`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{}' });
    if (!response.ok) setError('The desktop could not be ended.');
    await refresh();
  }
  const visibleDesktops = desktops.filter(d => sessionKind === 'console' ? d.kind === 'console' : d.kind !== 'console');
  return <div className="console-shell">
    <header className="console-header"><a className="brand" href="/"><img src="/donkeywork.png" alt=""/><span>DonkeyWork<small>DESKTOP</small></span></a><a href={brokerHome}>Desktop broker</a><button onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}>{theme === 'dark' ? 'Light' : 'Dark'}</button></header>
    <main className="console-main broker-main">
      <DeviceContext/>
      <div className="console-heading"><div><span className="eyebrow accent">YOUR WORKSPACE</span><h1>{sessionKind === 'console' ? 'Console sessions' : 'Terminal services'}</h1><p>{sessionKind === 'console' ? 'Connect to the desktop already running on this machine.' : 'Create or reconnect to an independent desktop. The physical console is unchanged.'}</p></div></div>
      <nav className="console-actions" aria-label="Session type">{[['console','Console sessions'],['terminal','Terminal services']].map(([kind,label]) => <button key={kind} aria-pressed={sessionKind === kind} onClick={() => {setSessionKind(kind); const url = new URL(location.href); url.searchParams.set('view',kind); window.history.replaceState(null,'',url);}}>{label}</button>)}</nav>
      {sessionKind === 'terminal' && <section className="broker-create" aria-label="Create desktop"><div><h2>Create another desktop</h2><p>Choose an environment. Existing desktops stay running.</p></div>
        <label>Desktop environment<select aria-label="Desktop environment" value={environment} onChange={e => setEnvironment(e.target.value)}>{environments.map(e => <option key={e.id} value={e.id}>{e.name}{e.validation !== 'validated' ? ' · validation pending' : ''}</option>)}</select></label>
        <button disabled={busy || !environment || !loaded} onClick={create}>{busy ? 'Creating…' : 'Create desktop'}</button>
      </section>}
      {!loaded ? <p>Loading desktops…</p> : visibleDesktops.length === 0 ? <section className="broker-empty"><h2>{sessionKind === 'console' ? 'No console available' : 'No managed desktops'}</h2><p>{sessionKind === 'console' ? 'The desktop agent must be running in a logged-in desktop session.' : 'Create a desktop above when you’re ready. Nothing starts automatically.'}</p></section> : <div className="broker-list">{visibleDesktops.map(d => <article className="broker-card" key={d.id} data-desktop-id={d.id}><div><h2>{environments.find(e => e.id === d.environment)?.name ?? d.environment}</h2><p>{d.user} · {d.kind === 'console' ? 'Existing console' : 'Managed desktop'} · {d.id.slice(0,8)}</p><span className="eyebrow">{d.state}</span></div><div className="console-actions"><button disabled={d.state !== 'ready'} onClick={() => reconnect(d.id)}>{d.kind === 'console' ? 'Open console' : 'Reconnect'}</button>{d.kind !== 'console' && <button onClick={() => close(d.id)}>End desktop</button>}</div></article>)}</div>}
      {notice && <p role="status">{notice}</p>}{error && <p role="alert">{error}</p>}
      {sessionKind === 'terminal' && history.some(d => d.state === 'failed') && <p role="status">A desktop failed to start. Its private service logs contain the diagnostic details.</p>}
      <footer className="console-status">Local-user lab broker · Trusted LAN only · Web authentication is not enabled</footer>
    </main>
  </div>;
}
