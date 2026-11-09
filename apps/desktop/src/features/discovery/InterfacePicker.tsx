import { useEffect, useState } from 'react';
import type { NetworkInterface } from '../../../../../packages/ipc-types/NetworkInterface';
import { networkInterfaces, message } from '../../lib/client';
export function InterfacePicker({ selected, onChange, disabled }: { selected: string[]; onChange: (selected: string[]) => void; disabled?: boolean }) {
  const [interfaces, setInterfaces] = useState<NetworkInterface[]>([]);
  const [error, setError] = useState('');
  useEffect(() => { void networkInterfaces().then(setInterfaces).catch(error => setError(message(error))); }, []);
  const unique = interfaces.filter((item, index, all) => all.findIndex(i => i.id === item.id) === index);
  return <fieldset className="interface-picker"><legend>Local network interfaces</legend><p className="field-help">Select your camera network. VPNs and virtual adapters are scanned only if selected.</p>{unique.map(item => <label className="interface-choice" key={item.id}><input type="checkbox" disabled={disabled} checked={selected.includes(item.id)} onChange={e => onChange(e.target.checked ? [...selected, item.id] : selected.filter(id => id !== item.id))} /><span><strong>{item.name}</strong><small>{item.address}/{item.prefix}{item.virtual_hint ? ' · Possibly virtual' : ''}</small></span></label>)}{unique.length === 0 && <p className="field-help">{error || 'No private IPv4 interfaces are available.'}</p>}</fieldset>;
}
