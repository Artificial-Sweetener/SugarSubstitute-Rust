//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Prove recommendation preview lifetimes independently of selection and acquisition state. */
import { createApp, h, nextTick, reactive } from 'vue';
import { ModelRecommendationPreview } from '../../../frontend/comfyui/ui/model_library/vue/ModelRecommendationPreview.js';
import { modelDiscoveryCss } from '../../../frontend/comfyui/ui/model_library/ModelDiscoveryStyles.js';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';

/** Mount real Vue preview behavior with a reactive provider URL boundary. */
function mount(url: string | null) {
  const root = document.createElement('div');
  const state = reactive<{ url: string | null }>({ url });
  const app = createApp({ render: () => h(ModelRecommendationPreview, { url: state.url }) });
  document.body.append(root);
  app.mount(root);
  return {
    root,
    state,
    dispose: () => {
      app.unmount();
      root.remove();
    },
  };
}

/** Require a real image rather than making absent preview nodes pass event assertions. */
function image(root: HTMLElement): HTMLImageElement {
  const element = root.querySelector('img');
  if (!element) throw new Error('Missing pending preview.');
  return element;
}

test('uses settled unavailable, loading, and ready states without creating markup from URL text', async () => {
  const fixture = mount(null);
  try {
    expect(fixture.root.textContent).toBe('Preview unavailable');
    expect(fixture.root.querySelector('img')).toBeNull();
    expect(fixture.root.querySelector('[role=status]')).toBeNull();
    fixture.state.url = '/literal-<b>preview</b>';
    await nextTick();
    expect(fixture.root.querySelector('b')).toBeNull();
    expect(image(fixture.root).getAttribute('src')).toBe('/literal-<b>preview</b>');
    expect(fixture.root.querySelector('[role=status]')?.textContent).toBe('Loading preview…');
    image(fixture.root).dispatchEvent(new Event('load'));
    await nextTick();
    expect(fixture.root.querySelector('[role=status]')).toBeNull();
    expect(fixture.root.querySelector('.sc-model-preview-unavailable')).toBeNull();
    expect(fixture.root.querySelector('img')).not.toBeNull();
  } finally {
    fixture.dispose();
  }
});

test('ignores stale terminal events after a provider URL changes', async () => {
  const fixture = mount('/first');
  try {
    const stale = image(fixture.root);
    fixture.state.url = '/second';
    await nextTick();
    stale.dispatchEvent(new Event('load'));
    stale.dispatchEvent(new Event('error'));
    await nextTick();
    expect(fixture.root.querySelector('[role=status]')).not.toBeNull();
    expect(image(fixture.root).getAttribute('src')).toBe('/second');
    image(fixture.root).dispatchEvent(new Event('load'));
    await nextTick();
    stale.dispatchEvent(new Event('error'));
    await nextTick();
    expect(fixture.root.querySelector('[role=status]')).toBeNull();
    expect(image(fixture.root).getAttribute('src')).toBe('/second');
  } finally {
    fixture.dispose();
  }
});

test('retries a revisited URL after failure and remains settled on late events', async () => {
  const fixture = mount('/first');
  try {
    const failed = image(fixture.root);
    failed.dispatchEvent(new Event('error'));
    await nextTick();
    failed.dispatchEvent(new Event('load'));
    await nextTick();
    expect(fixture.root.textContent).toBe('Preview unavailable');
    fixture.state.url = '/second';
    await nextTick();
    fixture.state.url = '/first';
    await nextTick();
    expect(fixture.root.querySelector('[role=status]')).not.toBeNull();
    image(fixture.root).dispatchEvent(new Event('load'));
    await nextTick();
    expect(fixture.root.querySelector('[role=status]')).toBeNull();
    expect(image(fixture.root).getAttribute('src')).toBe('/first');
    fixture.state.url = null;
    await nextTick();
    expect(fixture.root.textContent).toBe('Preview unavailable');
  } finally {
    fixture.dispose();
  }
});

test.each([
  ['discovery', 'sc-model-discovery-portrait', modelDiscoveryCss],
  ['version family', 'sc-model-version-portrait', modelPickerCss],
])(
  'keeps %s preview status above the title wash and inside its reserved area',
  async (_surface, portraitClass, css) => {
    const fixture = mount('/pending');
    const style = document.createElement('style');
    style.textContent = css;
    document.head.append(style);
    fixture.root.className = portraitClass;
    try {
      const loading = fixture.root.querySelector<HTMLElement>('.sc-model-preview-loading');
      const ring = fixture.root.querySelector<HTMLElement>('.sc-model-preview-busy-ring');
      const label = fixture.root.querySelector<HTMLElement>('.sc-model-preview-loading-label');
      expect(loading).not.toBeNull();
      expect(ring).not.toBeNull();
      expect(label).not.toBeNull();
      if (!loading || !ring || !label) throw new Error('Missing preview status elements.');
      expect(getComputedStyle(loading).zIndex).toBe('2');
      expect(getComputedStyle(ring).top).toBe('calc(32% - 17px)');
      expect(ring.namespaceURI).toBe('http://www.w3.org/2000/svg');
      expect(ring.getAttribute('viewBox')).toBe('0 0 34 34');
      expect(ring.querySelector('circle')?.getAttribute('r')).toBe('15');
      expect(ring.querySelector('circle')?.getAttribute('stroke-width')).toBe('4');
      expect(ring.querySelector('circle')?.getAttribute('pathLength')).toBe('360');
      expect(ring.querySelector('circle')?.getAttribute('stroke-dasharray')).toBe('105 255');
      expect(ring.querySelector('circle')?.getAttribute('stroke-linecap')).toBe('round');
      expect(getComputedStyle(ring).animation).toBe(
        'sc-model-busy-spin 1.2s steps(15,end) infinite',
      );
      expect(getComputedStyle(label).top).toBe('calc(32% + 20px)');
      expect(getComputedStyle(label).height).toBe('28px');
      image(fixture.root).dispatchEvent(new Event('error'));
      await nextTick();
      const unavailable = fixture.root.querySelector<HTMLElement>('.sc-model-preview-unavailable');
      if (!unavailable) throw new Error('Missing unavailable preview status.');
      const presentation = getComputedStyle(unavailable);
      expect(presentation.position).toBe('absolute');
      // JSDOM retains inset rather than resolving its longhands into portrait geometry.
      expect(presentation.getPropertyValue('inset')).toBe('0 16px 34px');
      expect(presentation.fontSize).toBe('12px');
      expect(presentation.zIndex).toBe('2');
      expect(getComputedStyle(fixture.root).backgroundColor).toBe('rgb(32, 32, 36)');
    } finally {
      style.remove();
      fixture.dispose();
    }
  },
);
