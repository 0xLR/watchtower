import { InterfacePicker } from '../discovery/InterfacePicker';
import { useState } from 'react';
import type { Settings } from '../../../../../packages/ipc-types/Settings';
import { saveSettings, message } from '../../lib/client';
import { Button } from '../../design/primitives';
import { EnginePicker } from './EnginePicker';
export function SettingsForm({ settings, onSaved }: { settings: Settings; onSaved: () => void }) {
  const [value, setValue] = useState(settings);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const save = async () => { setBusy(true); try { await saveSettings(value); onSaved(); } catch (error) { setError(message(error)); } finally { setBusy(false); } };
  return <div className="stack"><p className="supporting">Preferences are stored on this computer.</p><EnginePicker value={value.media_engine ?? "go2rtc"} onChange={media_engine => { if (media_engine) setValue({ ...value, media_engine }); }} disabled={busy} /><label className="check-field"><input type="checkbox" checked={value.default_substream} onChange={e => setValue({ ...value, default_substream: e.target.checked })} />Use substreams in the grid</label><p className="field-help">Expanded cameras always use their main stream.</p><label className="check-field"><input type="checkbox" checked={value.background_sweep} onChange={e => setValue({ ...value, background_sweep: e.target.checked })} />Recheck local addresses every few minutes</label><p className="field-help">Uses additional network activity and battery. Recovery after stream failures is always enabled.</p><InterfacePicker selected={value.selected_interfaces} onChange={selected_interfaces => setValue({ ...value, selected_interfaces })} disabled={busy} />{error && <p className="error-line" role="alert">{error}</p>}<Button disabled={busy} onClick={() => void save()}>{busy ? 'Saving…' : 'Save preferences'}</Button></div>;
}
