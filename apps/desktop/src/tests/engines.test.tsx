import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { EnginePicker } from '../features/settings/EnginePicker';
import { EngineAvailability } from '../features/settings/EngineAvailability';

describe('included media engines', () => {
  it('offers only Retina in a native build', () => {
    const html = renderToStaticMarkup(<EnginePicker value="retina" onChange={() => {}} />);
    expect(html).toContain('value="retina"');
    expect(html).not.toContain('value="go2rtc"');
    expect(html).not.toContain('value="mediamtx"');
  });
  it('explains a saved unavailable engine without offering it as selectable', () => {
    const html = renderToStaticMarkup(<EnginePicker value="go2rtc" onChange={() => {}} />);
    expect(html).toMatch(/value="go2rtc" disabled=""/);
    expect(html).toContain('not included; using Retina');
  });
  it('offers optional engines only when the backend includes them', () => {
    const html = renderToStaticMarkup(<EngineAvailability.Provider value={['retina', 'mediamtx']}><EnginePicker value="retina" onChange={() => {}} /></EngineAvailability.Provider>);
    expect(html).toContain('value="mediamtx"');
    expect(html).not.toContain('value="go2rtc"');
  });
});
