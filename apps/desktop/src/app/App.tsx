import { listen } from '@tauri-apps/api/event';
import type { AddressChanged } from '../../../../packages/ipc-types/AddressChanged';
import { DiscoveryPanel } from '../features/discovery/DiscoveryPanel';
import type { ObservedDevice } from '../../../../packages/ipc-types/ObservedDevice';
import { useCallback, useEffect, useState } from 'react';
import type { AppSnapshot } from '../../../../packages/ipc-types/AppSnapshot';
import { getSnapshot, message, playback, reportPlayerState, reorderCameras, saveSettings } from '../lib/client';
import { CameraGrid } from '../features/grid/CameraGrid';
import type { Layout } from '../features/grid/layout';
import { CameraForm } from '../features/cameras/CameraForm';
import { SettingsForm } from '../features/settings/SettingsForm';
import { Button, Readout } from '../design/primitives';
import { Dialog } from '../design/Dialog';
import { useWindowPaused } from '../lib/useWindowPaused';
import type { PlayerState } from '../player/StreamPlayer';

export function App() {
  const [discovered, setDiscovered] = useState<ObservedDevice>();
  const [snapshot, setSnapshot] = useState<AppSnapshot>();
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [panel, setPanel] = useState<'add' | 'settings' | 'cameras' | string>();
  const [statuses, setStatuses] = useState<Record<string, PlayerState>>({});
  const [clock, setClock] = useState('');
  const paused = useWindowPaused();
  const refresh = useCallback(async () => { try { setSnapshot(await getSnapshot()); setError(''); } catch (error) { setError(message(error)); } }, []);
  useEffect(() => { void refresh(); const timer = setInterval(() => setClock(new Date().toLocaleTimeString([], { hour12: false })), 1000); return () => clearInterval(timer); }, [refresh]);
  useEffect(() => {
    let disposed = false;
    const cleanups: (() => void)[] = [];
    void listen<AddressChanged>('camera:address-changed', event => {
      setNotice(`Camera address updated: ${event.payload.old_host} → ${event.payload.new_host}`); void refresh();
    }).then(unlisten => disposed ? unlisten() : cleanups.push(unlisten));
    void listen<string>('camera:recovery-notice', event => setNotice(event.payload)).then(unlisten => disposed ? unlisten() : cleanups.push(unlisten));
    return () => { disposed = true; cleanups.forEach(unlisten => unlisten()); };
  }, [refresh]);
  useEffect(() => { if (!notice) return; const timer = setTimeout(() => setNotice(''), 10000); return () => clearTimeout(timer); }, [notice]);
  const changed = () => { setPanel(undefined); setNotice('Camera configuration updated.'); void refresh(); };
  const state = useCallback((id: string, value: PlayerState, main: boolean) => { setStatuses(old => old[id] === value ? old : { ...old, [id]: value }); void reportPlayerState(id, main, value === 'paused' ? 'idle' : value).catch(() => {}); }, []);
  const reorder = async (ids: string[]) => { try { await reorderCameras(ids); await refresh(); } catch (error) { setError(message(error)); } };
  const layout = async (value: Layout) => { if (!snapshot) return; try { await saveSettings({ ...snapshot.settings, layout: value }); await refresh(); } catch (error) { setError(message(error)); } };
  const edit = snapshot?.cameras.find(view => view.camera.id === panel);
  const count = snapshot?.cameras.length ?? 0;
  const live = snapshot?.cameras.filter(view => statuses[view.camera.id] === 'live').length ?? 0;
  return <div className="shell">
    <header className="topbar"><div className="wordmark">WATCHTOWER<span>LOCAL CAMERA MONITOR</span></div><div className="header-readouts"><Readout label="LIVE" value={`${String(live).padStart(2, '0')} / ${String(count).padStart(2, '0')}`} /><Readout label="LOCAL TIME" value={clock || '—'} /></div><nav aria-label="Configuration"><button className="text-button" onClick={() => setPanel('cameras')}>Cameras</button><button className="text-button" onClick={() => setPanel('settings')}>Settings</button></nav></header>
    <main><div className="section-heading"><div><p className="eyebrow">MONITOR / 01</p><h1>Live view</h1></div><div className="toolbar"><label className="layout-select">Layout<select aria-label="Grid layout" value={snapshot?.settings.layout ?? 'auto'} onChange={e => void layout(e.target.value as Layout)}><option value="auto">Fit to count</option><option value="1">1 column</option><option value="2">2 × 2</option><option value="3">3 × 3</option><option value="4">4 × 4</option></select></label><Button onClick={() => setPanel('discovery')}>Find cameras</Button><Button onClick={() => { setDiscovered(undefined); setPanel('add'); }}>+ Add camera</Button></div></div>
      {error && <div className="banner error-line" role="alert">{error}<button className="text-button" onClick={() => void refresh()}>Retry</button></div>}
      {snapshot?.warning && <p className="banner error-line" role="alert">{snapshot.warning}</p>}
      {notice && <div className="banner" role="status">{notice}<button className="text-button" onClick={() => setNotice('')} aria-label="Dismiss notice">×</button></div>}
      {count > 0 && snapshot ? <CameraGrid key={snapshot.settings.media_engine} cameras={snapshot.cameras.map(view => view.camera)} layout={snapshot.settings.layout as Layout} substreams={snapshot.settings.default_substream} paused={paused} playback={playback} onReorder={ids => void reorder(ids)} onEdit={setPanel} onState={state} /> : <section className="tile empty-grid" aria-label="Live camera grid"><div className="empty-state"><span className="crosshair" aria-hidden="true">+</span><p className="eyebrow">YOUR NETWORK. YOUR VIEW.</p><h2>A quiet place to keep watch.</h2><p>Add your first camera to bring this view online.<br />Your streams and credentials stay on this computer.</p><div className="actions empty-actions"><Button onClick={() => setPanel('discovery')}>Find cameras</Button><Button onClick={() => { setDiscovered(undefined); setPanel('add'); }}>Add manually</Button></div><p className="field-help">RTSP streams · Tapo camera accounts · Local network</p></div></section>}
    </main>
    <footer><span>WATCHTOWER / 0.2.0</span><span>{paused ? 'PLAYBACK PAUSED' : 'LOCAL ONLY'}</span></footer>
    {panel && <Dialog title={panel === 'add' ? 'Add camera' : panel === 'discovery' ? 'Find cameras' : panel === 'settings' ? 'Settings' : panel === 'cameras' ? 'Your cameras' : 'Edit camera'} onClose={() => setPanel(undefined)}>
      {(panel === 'add' || edit) && <CameraForm key={edit?.camera.id ?? discovered?.host ?? 'new'} initialHost={discovered ? `${discovered.host}:${discovered.port || 554}` : undefined} initialName={discovered?.name ?? discovered?.model ?? undefined} discovered={!!discovered?.endpoint_uuid} camera={edit?.camera} onSaved={changed} onCancel={() => setPanel(undefined)} />}
      {panel === 'discovery' && snapshot && <DiscoveryPanel settings={snapshot.settings} onAdd={device => { setDiscovered(device); setPanel('add'); }} />}
      {panel === 'settings' && snapshot && <SettingsForm settings={snapshot.settings} onSaved={() => { setPanel(undefined); void refresh(); }} />}
      {panel === 'cameras' && <div className="stack">{count === 0 && <p className="supporting">No cameras configured.</p>}{snapshot?.cameras.map(view => <div className="camera-list-row" key={view.camera.id}><div><strong>{view.camera.name}</strong><p>{view.camera.host}:{view.camera.port}</p><span className="field-help">{view.tracking ? 'Address tracking enabled' : view.camera.is_local ? view.identity.mac || view.identity.onvif_endpoint_uuid || view.identity.serial ? 'Choose a camera network in Settings to enable address tracking' : 'Address tracking unavailable: no stable identity found' : 'Remote host: address tracking off'}</span></div><Button onClick={() => setPanel(view.camera.id)}>Edit</Button></div>)}<Button onClick={() => { setDiscovered(undefined); setPanel('add'); }}>+ Add camera</Button></div>}
    </Dialog>}
  </div>;
}
