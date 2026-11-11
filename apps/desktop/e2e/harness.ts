// Test-only entrypoint. Excluded from Vite's production build.
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import type { CameraView } from '../../../packages/ipc-types/CameraView';
import type { CameraInput } from '../../../packages/ipc-types/CameraInput';
import type { Settings } from '../../../packages/ipc-types/Settings';
import type { Playback } from '../../../packages/ipc-types/Playback';
const count = Number(new URL(location.href).searchParams.get('count') ?? 0);
const makeCamera = (index: number): CameraView => ({ camera: { id: `fixture-${index}`, name: `Test camera ${String(index + 1).padStart(2, '0')}`, created_at: 1, sort_order: index, host: '127.0.0.1', port: 8554, path_main: '/watchtower-test', path_sub: new URL(location.href).searchParams.has('no_sub') ? null : '/watchtower-sub', username: '', source_kind: 'rtsp', is_local: false }, identity: { camera_id: `fixture-${index}`, mac: null, onvif_endpoint_uuid: null, serial: null, model: null, last_seen_at: null, last_seen_host: null }, tracking: false, status: 'idle' });
let cameras = Array.from({ length: count }, (_, index) => makeCamera(index));
let settings: Settings = { media_engine: 'go2rtc', layout: 'auto', default_substream: true, background_sweep: false, selected_interfaces: [] };
let tested: CameraInput | undefined;
let minimized = false;
let streams: { main: Playback; sub: Playback } | undefined;
try { const response = await fetch('/e2e/playback.json'); if (response.ok) streams = await response.json(); } catch { /* UI-only tests can run without media */ }
settings.media_engine = streams?.main.engine ?? 'go2rtc';
Object.assign(window, { __setMinimized: (value: boolean) => { minimized = value; }, __lastPlayback: null, __hasMedia: !!streams });
mockWindows('main');
mockIPC((command, args) => {
  const payload = args as Record<string, unknown>;
  switch (command) {
    case 'snapshot': return { cameras, settings, media_ready: true, warning: null };
    case 'app_info': return { version: '0.1.0' };
    case 'playback': Object.assign(window, { __lastPlayback: payload }); return streams?.[payload.main ? 'main' : 'sub'] ?? { websocket_url: 'ws://127.0.0.1:1/unavailable', hls_url: 'http://127.0.0.1:1/unavailable' };
    case 'test_camera': tested = payload.input as CameraInput; return 'connection-receipt';
    case 'save_camera': {
      if (!tested || payload.token !== 'connection-receipt') throw new Error('Test required');
      const view = makeCamera(cameras.length); view.camera.name = tested.name; view.camera.media_engine = tested.media_engine; cameras.push(view); tested = undefined; return view.camera.id;
    }
    case 'delete_camera': cameras = cameras.filter(view => view.camera.id !== payload.id); return;
    case 'save_settings': settings = payload.settings as Settings; return;
    case 'reorder_cameras': cameras = (payload.ids as string[]).map(id => cameras.find(view => view.camera.id === id)!); return;
    case 'network_interfaces': return [{ id: 'test', name: 'Test Ethernet', address: '192.168.1.10', prefix: 24, virtual_hint: false }];
    case 'scan_cameras': return { devices: new URL(location.href).searchParams.has('found') ? [{ host: '127.0.0.1', port: 8554, name: 'Discovered test', model: null, serial: null, mac: null, endpoint_uuid: null, onvif_port: null }] : [], warnings: [] };
    case 'plugin:window|is_minimized': return minimized;
    case 'plugin:window|is_visible': return true;
    case 'plugin:event|listen': return 1;
    default: return null;
  }
}, { shouldMockEvents: true });
await import('../src/main');
