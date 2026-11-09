import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { App } from '../app/App';
describe('desktop shell', () => {
  it('provides camera setup without fake live feeds', () => {
    const html = renderToStaticMarkup(<App />);
    expect(html).toContain('Add your first camera');
    expect(html).toContain('Live camera grid');
    expect(html).toContain('Settings');
  });
});
