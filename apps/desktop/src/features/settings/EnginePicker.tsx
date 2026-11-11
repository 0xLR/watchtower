import type { MediaEngine } from '../../../../../packages/ipc-types/MediaEngine';
export function EnginePicker({ value, onChange, inherit = false, disabled = false }: { value: MediaEngine | null; onChange: (value: MediaEngine | null) => void; inherit?: boolean; disabled?: boolean }) {
  return <div><label className="field"><span>Media engine</span><select aria-label="Media engine" value={value ?? ''} disabled={disabled} onChange={event => onChange(event.target.value ? event.target.value as MediaEngine : null)}>
    {inherit && <option value="">Use default from Settings</option>}
    <option value="retina">Retina — native Rust / H.264 WebRTC</option>
    <option value="go2rtc">go2rtc — WebRTC / MSE / HLS</option>
    <option value="mediamtx">MediaMTX — WebRTC / HLS</option>
  </select></label><p className="field-help">Retina runs inside Watchtower and currently supports H.264 video. Camera overrides take priority over the default.</p></div>;
}
