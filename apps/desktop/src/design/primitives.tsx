import type { ButtonHTMLAttributes, InputHTMLAttributes, PropsWithChildren } from 'react';

export function Panel({ children }: PropsWithChildren) { return <section className="panel">{children}</section>; }
export function Button(props: ButtonHTMLAttributes<HTMLButtonElement>) { return <button {...props} className={`button ${props.className ?? ''}`} />; }
export function Field({ label, ...props }: InputHTMLAttributes<HTMLInputElement> & { label: string }) { return <label className="field"><span>{label}</span><input {...props} /></label>; }
export function Readout({ label, value }: { label: string; value: string }) { return <div className="readout"><span>{label}</span><output>{value}</output></div>; }
export function StatusLine({ children }: PropsWithChildren) { return <div className="status-line">{children}</div>; }
export function Tile({ children }: PropsWithChildren) { return <section className="tile" aria-label="Camera grid">{children}</section>; }
