import { useEffect, useRef, type PropsWithChildren } from 'react';

export function Dialog({ title, children, onClose }: PropsWithChildren<{ title: string; onClose: () => void }>) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { ref.current?.showModal(); return () => { ref.current?.close(); }; }, []);
  return <dialog className="drawer" ref={ref} onCancel={event => { event.preventDefault(); onClose(); }} aria-labelledby="drawer-title">
    <header className="drawer-header"><div><span className="eyebrow">WATCHTOWER / CONFIGURATION</span><h2 id="drawer-title">{title}</h2></div><button className="icon-button" aria-label="Close panel" onClick={onClose}>×</button></header>
    <div className="drawer-body">{children}</div>
  </dialog>;
}
