import { useCallback, useEffect, useState, type CSSProperties } from 'react';
import type { Playback } from '../../../../../packages/ipc-types/Playback';
import { StreamPlayer, type PlayerState } from '../../player/StreamPlayer';
import { columnsFor, reorder, type Layout } from './layout';

type GridCamera = { id: string; name: string; host: string; media_engine?: string | null };
type GridProps = {
  cameras: GridCamera[]; layout: Layout; paused: boolean; substreams: boolean;
  playback: (id: string, main: boolean) => Promise<Playback>;
  onReorder: (ids: string[]) => void; onEdit: (id: string) => void;
  onState: (id: string, state: PlayerState, main: boolean) => void;
};
function CameraTile({ camera, expanded, paused, main, playback, onState, onExpand, onEdit, move, index }: {
  camera: GridCamera; expanded: boolean; paused: boolean; main: boolean; index: number;
  playback: GridProps['playback']; onState: GridProps['onState']; onExpand: () => void; onEdit: () => void; move: (offset: number) => void;
}) {
  const [status, setStatus] = useState<PlayerState>('connecting');
  const load = useCallback(() => playback(camera.id, main), [playback, camera.id, main]);
  const state = useCallback((value: PlayerState) => { setStatus(value); onState(camera.id, value, main); }, [camera.id, onState, main]);
  return <article className={`camera-tile ${expanded ? 'is-expanded' : ''}`}>
    <header className="camera-header"><div className="camera-title"><span className="camera-index">{String(index + 1).padStart(2, '0')}</span><button className="text-button camera-name" onClick={onExpand} aria-label={`${expanded ? 'Collapse' : 'Expand'} ${camera.name}`}>{camera.name}</button></div><div className="tile-actions"><button className="icon-button" onClick={() => move(-1)} aria-label={`Move ${camera.name} earlier`}>←</button><button className="icon-button" onClick={() => move(1)} aria-label={`Move ${camera.name} later`}>→</button><button className="icon-button" onClick={onEdit} aria-label={`Edit ${camera.name}`}>···</button></div></header>
    <div className="video-stage" onClick={onExpand}><StreamPlayer load={load} paused={paused} onState={state} /></div>
    <div className="camera-footer"><span className={`camera-status ${status === 'live' ? 'live' : status === 'unreachable' ? 'fault' : ''}`}><i />{status === 'live' ? 'LIVE' : status.toUpperCase()}</span><span>{new Date().toLocaleTimeString([], { hour12: false })} / {main ? 'MAIN' : 'SUB'} / {camera.host}</span></div>
  </article>;
}
export function CameraGrid(props: GridProps) {
  const [expanded, setExpanded] = useState<string | null>(null);
  const [dragged, setDragged] = useState<string>();
  useEffect(() => {
    const key = (event: KeyboardEvent) => { if (event.key === 'Escape' && !document.querySelector('dialog[open]')) setExpanded(null); };
    window.addEventListener('keydown', key); return () => window.removeEventListener('keydown', key);
  }, []);
  useEffect(() => { if (!props.cameras.some(c => c.id === expanded)) setExpanded(null); }, [props.cameras, expanded]);
  const cameras = expanded ? props.cameras.filter(c => c.id === expanded) : props.cameras;
  return <section aria-label="Live camera grid" className={`camera-grid ${expanded ? 'expanded-grid' : ''}`} style={{ '--columns': expanded ? 1 : columnsFor(props.cameras.length, props.layout) } as CSSProperties}>
    {expanded && <button className="button return-grid" onClick={() => setExpanded(null)}>← Return to grid <kbd>Esc</kbd></button>}
    {cameras.map(camera => {
      const index = props.cameras.findIndex(c => c.id === camera.id);
      return <div key={`${camera.id}:${camera.host}:${camera.media_engine ?? "default"}`} draggable={!expanded} onDragStart={event => { setDragged(camera.id); event.dataTransfer.setData('text/plain', camera.id); event.dataTransfer.effectAllowed = 'move'; }} onDragOver={event => event.preventDefault()} onDrop={event => { event.preventDefault(); if (dragged) props.onReorder(reorder(props.cameras.map(c => c.id), dragged, camera.id)); setDragged(undefined); }}>
        <CameraTile camera={camera} index={index} expanded={expanded === camera.id} main={expanded === camera.id || !props.substreams} paused={props.paused} playback={props.playback} onState={props.onState} onExpand={() => setExpanded(expanded ? null : camera.id)} onEdit={() => props.onEdit(camera.id)} move={offset => { const target = props.cameras[index + offset]; if (target) props.onReorder(reorder(props.cameras.map(c => c.id), camera.id, target.id)); }} />
      </div>;
    })}
  </section>;
}
