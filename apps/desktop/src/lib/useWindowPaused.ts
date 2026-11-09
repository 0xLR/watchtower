import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

export function useWindowPaused() {
  const [paused, setPaused] = useState(false);
  useEffect(() => {
    let disposed = false;
    const check = async () => {
      let hidden = document.hidden;
      try { const window = getCurrentWindow(); hidden ||= await window.isMinimized() || !await window.isVisible(); } catch { /* document visibility remains the fallback */ }
      if (!disposed) setPaused(hidden);
    };
    document.addEventListener('visibilitychange', check);
    const timer = setInterval(() => void check(), 1000); void check();
    return () => { disposed = true; clearInterval(timer); document.removeEventListener('visibilitychange', check); };
  }, []);
  return paused;
}
