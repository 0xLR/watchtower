import { useState } from 'react';
import type { Settings } from '../../../../../packages/ipc-types/Settings';
import type { ScanResult } from '../../../../../packages/ipc-types/ScanResult';
import type { ObservedDevice } from '../../../../../packages/ipc-types/ObservedDevice';
import { scanCameras, saveSettings, message } from '../../lib/client';
import { InterfacePicker } from './InterfacePicker';
import { Button } from '../../design/primitives';
export function DiscoveryPanel({ settings, onAdd }: { settings: Settings; onAdd: (device: ObservedDevice) => void }) {
  const [selected, setSelected] = useState(settings.selected_interfaces);
  const [sweep, setSweep] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ScanResult>();
  const [error, setError] = useState('');
  const scan = async () => {
    setBusy(true); setError(''); setResult(undefined);
    try { await saveSettings({ ...settings, selected_interfaces: selected }); setResult(await scanCameras(selected, sweep)); } catch (error) { setError(message(error)); } finally { setBusy(false); }
  };
  return <div className="stack"><p className="supporting">Find ONVIF cameras on your selected local networks. Camera accounts are requested when adding a result.</p><InterfacePicker selected={selected} onChange={setSelected} disabled={busy} /><label className="check-field"><input type="checkbox" checked={sweep} disabled={busy} onChange={e => setSweep(e.target.checked)} />Also probe RTSP and Tapo ports</label><p className="field-help">Fallback probes at most 254 addresses per interface. Large networks are capped to the interface's /24. Select at most four interfaces per scan.</p><Button disabled={busy || selected.length === 0 || selected.length > 4} onClick={() => void scan()}>{busy ? 'Scanning local networks…' : result ? 'Scan again' : 'Find cameras'}</Button>{error && <p className="error-line" role="alert">{error}</p>}{result && <><p className="eyebrow" role="status">{result.devices.length} {result.devices.length === 1 ? 'device' : 'devices'} found</p>{result.warnings.map((warning, index) => <p className="field-help" key={index}>{warning}</p>)}{result.devices.length === 0 && <p className="supporting">No cameras replied. Enable the port-probe fallback, verify the selected interface, or add an address manually.</p>}{result.devices.map(device => <div className="camera-list-row" key={device.host}><div><strong>{device.name || device.model || (device.endpoint_uuid ? 'ONVIF camera' : 'RTSP candidate')}</strong><p>{device.host}{device.model ? ` / ${device.model}` : ''}</p><span className="field-help">{device.endpoint_uuid ? 'Identity advertised' : 'Connection test required'}</span></div><Button onClick={() => onAdd(device)}>Add</Button></div>)}</>}</div>;
}
