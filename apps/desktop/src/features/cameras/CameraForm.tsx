import { useState, type FormEvent } from 'react';
import type { Camera } from '../../../../../packages/ipc-types/Camera';
import type { CameraInput } from '../../../../../packages/ipc-types/CameraInput';
import { testCamera, saveCamera, deleteCamera, message } from '../../lib/client';
import { Button, Field } from '../../design/primitives';

export function CameraForm({ camera, initialHost, initialName, discovered, onSaved, onCancel }: { camera?: Camera; initialHost?: string; initialName?: string; discovered?: boolean; onSaved: () => void; onCancel: () => void }) {
  const [input, setInput] = useState<CameraInput>({ id: camera?.id ?? null, name: camera?.name ?? initialName ?? '', address: camera ? `${camera.host.includes(':') ? `[${camera.host}]` : camera.host}:${camera.port}` : initialHost ?? '', username: camera?.username ?? '', password: null, path_main: camera?.path_main ?? '/stream1', path_sub: camera ? camera.path_sub ?? '' : '/stream2', source_kind: camera?.source_kind ?? (discovered ? 'onvif_discovered' : null) });
  const [emptyPassword, setEmptyPassword] = useState(false);
  const [token, setToken] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [confirmDelete, setConfirmDelete] = useState(false);
  const change = (value: Partial<CameraInput>) => { setInput(old => ({ ...old, ...value })); setToken(undefined); setError(''); };
  const test = async (event: FormEvent) => {
    event.preventDefault(); setBusy(true); setError(''); setToken(undefined);
    try { setToken(await testCamera(input)); } catch (error) { setError(message(error)); } finally { setBusy(false); }
  };
  const save = async () => {
    if (!token) return; setBusy(true); setError('');
    try { await saveCamera(token); setInput(old => ({ ...old, password: null, address: '' })); onSaved(); } catch (error) { setError(message(error)); setToken(undefined); } finally { setBusy(false); }
  };
  const remove = async () => {
    if (!camera) return; setBusy(true);
    try { await deleteCamera(camera.id); onSaved(); } catch (error) { setError(message(error)); } finally { setBusy(false); }
  };
  return <form onSubmit={test} className="stack">
    <p className="supporting">Use the camera's local RTSP account. For Tapo, enable Camera Account in its advanced settings first.</p>
    <Field label="Camera name" required maxLength={120} autoFocus value={input.name} onChange={e => change({ name: e.target.value })} disabled={busy} placeholder="Front entrance" />
    <Field label="Camera address or full RTSP URL" required type="password" autoComplete="off" spellCheck={false} value={input.address} onChange={e => change({ address: e.target.value, ...(e.target.value.includes('://') ? { path_main: null, path_sub: null } : {}) })} disabled={busy} placeholder="192.168.1.30 or rtsp://…" />
    <p className="field-help">Masked because a pasted URL may contain credentials. Saved addresses appear in the camera list.</p>
    <div className="field-row"><Field label="Username" autoComplete="off" value={input.username} onChange={e => change({ username: e.target.value })} disabled={busy} /><Field label={camera ? 'Password (blank keeps saved value)' : 'Password'} type="password" autoComplete="new-password" value={input.password ?? ''} onChange={e => change({ password: e.target.value || null })} disabled={busy || emptyPassword} /></div>
    {camera && <label className="check-field"><input type="checkbox" checked={emptyPassword} disabled={busy} onChange={e => { setEmptyPassword(e.target.checked); change({ password: e.target.checked ? '' : null }); }} />Use an empty password</label>}
    <details><summary>Stream paths</summary><div className="stack"><Field label="Main stream path" value={input.path_main ?? ''} onChange={e => change({ path_main: e.target.value })} placeholder="From URL, or /stream1" disabled={busy} /><Field label="Substream path (empty uses main)" value={input.path_sub ?? ''} onChange={e => change({ path_sub: e.target.value })} placeholder="/stream2" disabled={busy} /></div></details>
    {error && <p className="error-line" role="alert">{error}</p>}
    {token && <p className="success-line" role="status">Connection verified. Ready to save.</p>}
    <div className="actions"><Button type="submit" disabled={busy}>{busy ? 'Working…' : 'Test connection'}</Button><Button type="button" disabled={busy || !token} onClick={() => void save()}>Save camera</Button><Button type="button" disabled={busy} onClick={onCancel}>Cancel</Button></div>
    {camera && <div className="danger-zone"><p>Remove this camera and its saved credentials.</p>{confirmDelete ? <div className="actions"><Button type="button" disabled={busy} onClick={() => void remove()}>Confirm removal</Button><Button type="button" onClick={() => setConfirmDelete(false)}>Keep camera</Button></div> : <button type="button" className="text-button fault" onClick={() => setConfirmDelete(true)}>Remove camera</button>}</div>}
  </form>;
}
