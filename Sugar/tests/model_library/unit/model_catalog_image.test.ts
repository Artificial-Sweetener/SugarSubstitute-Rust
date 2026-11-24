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
/** Safeguard recovered prepared images without remounting successful images or host controls. */
import { createApp, h, nextTick, reactive } from 'vue';
import type { ModelThumbnail } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import { ModelCatalogImage } from '../../../frontend/comfyui/ui/model_library/vue/ModelCatalogImage.js';

/** Mount a real image component behind the immutable catalog item boundary. */
function mount(fallback = true) {
  const root = document.createElement('div');
  const state = reactive<{ image: ModelThumbnail }>({
    image: {
      assetId: 'a'.repeat(64),
      role: 'standard',
      width: 128,
      height: 128,
      url: '/assets/first',
    },
  });
  const app = createApp({
    render: () =>
      h(ModelCatalogImage, { image: state.image, fallback, class: 'fixture-image', lazy: true }),
  });
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

/** Require an actual image for terminal-event assertions. */
function image(root: HTMLElement): HTMLImageElement {
  const value = root.querySelector('img');
  if (!value) throw new Error('Missing prepared image.');
  return value;
}

test('retries the same asset URL after failure when the catalog publishes new evidence', async () => {
  const fixture = mount();
  try {
    const failed = image(fixture.root);
    failed.dispatchEvent(new Event('error'));
    await nextTick();
    expect(fixture.root.textContent).toBe('No image');
    fixture.state.image = { ...fixture.state.image };
    await nextTick();
    const recovered = image(fixture.root);
    expect(recovered).not.toBe(failed);
    expect(recovered.getAttribute('src')).toBe('/assets/first');
    failed.dispatchEvent(new Event('error'));
    await nextTick();
    expect(image(fixture.root)).toBe(recovered);
  } finally {
    fixture.dispose();
  }
});

test('retains loaded image elements when unrelated catalog metadata changes', async () => {
  const fixture = mount();
  try {
    const loaded = image(fixture.root);
    loaded.dispatchEvent(new Event('load'));
    fixture.state.image = { ...fixture.state.image };
    await nextTick();
    expect(image(fixture.root)).toBe(loaded);
    expect(loaded.classList.contains('fixture-image')).toBe(true);
    expect(loaded.getAttribute('loading')).toBe('lazy');
  } finally {
    fixture.dispose();
  }
});

test('ignores errors from replaced asset URLs and renders attribute text literally', async () => {
  const fixture = mount();
  try {
    const stale = image(fixture.root);
    fixture.state.image = { ...fixture.state.image, url: '/assets/<b>literal</b>' };
    await nextTick();
    stale.dispatchEvent(new Event('error'));
    await nextTick();
    expect(image(fixture.root).getAttribute('src')).toBe('/assets/<b>literal</b>');
    expect(fixture.root.querySelector('b')).toBeNull();
  } finally {
    fixture.dispose();
  }
});

test('removes a failed decorative banner and restores it on publication', async () => {
  const fixture = mount(false);
  try {
    image(fixture.root).dispatchEvent(new Event('error'));
    await nextTick();
    expect(fixture.root.textContent).toBe('');
    expect(fixture.root.querySelector('img')).toBeNull();
    fixture.state.image = { ...fixture.state.image };
    await nextTick();
    expect(image(fixture.root).classList.contains('fixture-image')).toBe(true);
  } finally {
    fixture.dispose();
  }
});

/** The image being loaded out of sight for a replacement. */
function loading(root: HTMLElement): HTMLImageElement | null {
  return root.querySelector<HTMLImageElement>('img[data-sc-model-image-loading]');
}

/** The image a person sees. */
function visible(root: HTMLElement): HTMLImageElement {
  const value = root.querySelector<HTMLImageElement>('img:not([data-sc-model-image-loading])');
  if (!value) throw new Error('Missing visible image.');
  return value;
}

test('keeps the image on screen until its replacement has loaded', async () => {
  const fixture = mount();
  try {
    const shown = visible(fixture.root);
    shown.dispatchEvent(new Event('load'));
    await nextTick();
    fixture.state.image = { ...fixture.state.image, url: '/assets/second' };
    await nextTick();
    expect(visible(fixture.root)).toBe(shown);
    expect(shown.getAttribute('src')).toBe('/assets/first');
    expect(loading(fixture.root)?.getAttribute('src')).toBe('/assets/second');
    loading(fixture.root)?.dispatchEvent(new Event('load'));
    await nextTick();
    expect(visible(fixture.root)).toBe(shown);
    expect(shown.getAttribute('src')).toBe('/assets/second');
    expect(loading(fixture.root)).toBeNull();
  } finally {
    fixture.dispose();
  }
});

test('a newer replacement supersedes one still loading, and a failed one keeps the image', async () => {
  const fixture = mount();
  try {
    const shown = visible(fixture.root);
    shown.dispatchEvent(new Event('load'));
    await nextTick();
    fixture.state.image = { ...fixture.state.image, url: '/assets/second' };
    await nextTick();
    const superseded = loading(fixture.root);
    fixture.state.image = { ...fixture.state.image, url: '/assets/third' };
    await nextTick();
    superseded?.dispatchEvent(new Event('load'));
    await nextTick();
    expect(shown.getAttribute('src')).toBe('/assets/first');
    expect(loading(fixture.root)?.getAttribute('src')).toBe('/assets/third');
    loading(fixture.root)?.dispatchEvent(new Event('error'));
    await nextTick();
    expect(visible(fixture.root)).toBe(shown);
    expect(shown.getAttribute('src')).toBe('/assets/first');
    expect(fixture.root.textContent).toBe('');
    expect(loading(fixture.root)).toBeNull();
    // The catalog publishing the model's image again is what retries the failed address.
    fixture.state.image = { ...fixture.state.image };
    await nextTick();
    expect(loading(fixture.root)?.getAttribute('src')).toBe('/assets/third');
  } finally {
    fixture.dispose();
  }
});
