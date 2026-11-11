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
/** Prove native dropdown presentation follows its control relationship through every opening path. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { AppearanceHost } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceHost.js';
import { NativeFieldAppearance } from '../../../frontend/comfyui/ui/surface/appearance/NativeFieldAppearance.js';
import { NativeInlineComboSearch } from '../../../frontend/comfyui/ui/surface/appearance/NativeInlineComboSearch.js';
import { NativeChoicePreparation } from '../../../frontend/comfyui/ui/surface/NativeChoicePreparation.js';
import { prepareNativePresentation } from '../../../frontend/comfyui/ui/surface/NativePresentationReady.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  jest.restoreAllMocks();
});

/** Mount a native field with Comfy-owned popup rendering and optional asynchronous choice discovery. */
function fixture(prepared: boolean) {
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  jest.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
  const errors = jest.fn();
  const host = new AppearanceHost({
    document,
    app: {
      ui: {
        settings: {
          addSetting() {},
          getSettingValue: (key: string) => key === 'Comfy.VueNodes.Enabled',
        },
      },
    },
    reportError: errors,
  });
  host.setup();
  host.setStyle('gimp');
  const root = document.createElement('div');
  root.className = 'sugarcubes-native-widget';
  const layout = document.createElement('div');
  layout.dataset.scStyleComboLayout = '';
  const caption = document.createElement('div');
  caption.textContent = 'Source';
  const frame = document.createElement('div');
  frame.dataset.scStyleComboFrame = '';
  frame.dataset.captureWheel = 'true';
  const trigger = document.createElement('button');
  trigger.setAttribute('role', 'combobox');
  trigger.setAttribute('aria-label', 'Source');
  trigger.setAttribute('aria-controls', 'source-popup');
  trigger.setAttribute('aria-expanded', 'false');
  trigger.dataset.testid = 'widget-select-default-trigger';
  const arrow = document.createElement('button');
  arrow.setAttribute('aria-hidden', 'true');
  const glyph = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  arrow.append(glyph);
  frame.append(trigger, arrow);
  layout.append(caption, frame);
  root.append(layout);
  document.body.append(root);
  const popup = document.createElement('div');
  popup.id = 'source-popup';
  popup.setAttribute('role', 'listbox');
  const shell = document.createElement('div');
  const search = document.createElement('input');
  search.dataset.testid = 'widget-select-default-search-input';
  shell.append(search);
  popup.append(shell);
  trigger.addEventListener('click', () => {
    trigger.setAttribute('aria-expanded', 'true');
    document.body.append(popup);
  });
  trigger.addEventListener('keydown', (event) => {
    if (event.key === 'ArrowDown') trigger.click();
  });
  arrow.addEventListener('click', () => trigger.click());
  const preparation = prepared ? new NativeChoicePreparation(root, async () => true) : null;
  prepareNativePresentation(root);
  frames.splice(0).forEach((callback) => callback(0));
  const present = jest.spyOn(NativeFieldAppearance.prototype, 'present');
  return {
    host,
    root,
    caption,
    trigger,
    glyph,
    popup,
    search,
    frames,
    present,
    errors,
    preparation,
  };
}

/** Deliver native mutation notifications and the appearance owner's coalesced presentation work. */
async function settle(frames: FrameRequestCallback[]): Promise<void> {
  for (let index = 0; index < 12; index++) await Promise.resolve();
  frames.splice(0).forEach((callback) => callback(0));
  for (let index = 0; index < 4; index++) await Promise.resolve();
}

test.each([false, true])(
  'styles every opening path with asynchronous preparation=%s',
  async (prepared) => {
    const f = fixture(prepared);
    try {
      for (const opening of ['caption', 'value', 'arrow', 'keyboard', 'programmatic'] as const) {
        f.popup.remove();
        f.trigger.setAttribute('aria-expanded', 'false');
        await settle(f.frames);
        if (opening === 'caption') {
          f.caption.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }));
          f.caption.click();
        } else if (opening === 'arrow') {
          f.glyph.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }));
          f.glyph.dispatchEvent(new MouseEvent('click', { bubbles: true, button: 0 }));
        } else if (opening === 'keyboard') {
          f.trigger.focus();
          f.trigger.dispatchEvent(
            new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }),
          );
        } else {
          if (opening === 'value')
            f.trigger.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }));
          f.trigger.click();
        }
        await settle(f.frames);
        expect({ opening, mounted: f.popup.isConnected }).toEqual({ opening, mounted: true });
        expect({ opening, styled: f.popup.hasAttribute('data-sc-style-popup') }).toEqual({
          opening,
          styled: true,
        });
        expect(f.popup.dataset.scMenuSurface).toBe('choices');
        expect(f.search.hasAttribute('data-sc-inline-search')).toBe(true);
        expect(f.root.querySelector('[role="combobox"]')).toBe(f.trigger);
      }
      expect(f.present).not.toHaveBeenCalled();
      expect(f.errors).not.toHaveBeenCalled();
    } finally {
      f.preparation?.dispose();
      f.host.dispose();
    }
    expect(f.popup.hasAttribute('data-sc-style-popup')).toBe(false);
    expect(f.popup.hasAttribute('data-sc-menu-surface')).toBe(false);
    expect(f.search.hasAttribute('data-sc-inline-search')).toBe(false);
  },
);

test('styles a late native popup relationship without an input gesture or unrelated card refresh', async () => {
  const f = fixture(false);
  try {
    f.trigger.removeAttribute('aria-controls');
    const unrelated = document.createElement('div');
    unrelated.id = 'unrelated-popup';
    unrelated.setAttribute('role', 'listbox');
    document.body.append(unrelated, f.popup);
    await settle(f.frames);
    expect(f.popup.hasAttribute('data-sc-style-popup')).toBe(false);
    f.trigger.setAttribute('aria-controls', f.popup.id);
    f.trigger.setAttribute('aria-expanded', 'true');
    await settle(f.frames);
    expect(f.popup.hasAttribute('data-sc-style-popup')).toBe(true);
    expect(unrelated.hasAttribute('data-sc-style-popup')).toBe(false);
    f.search.value = 'query';
    f.search.dispatchEvent(new Event('input', { bubbles: true }));
    await settle(f.frames);
    expect(f.present).not.toHaveBeenCalled();
    f.host.setStyle('comfy');
    expect(f.popup.hasAttribute('data-sc-style-popup')).toBe(false);
    f.host.setStyle('gimp');
    await settle(f.frames);
    expect(f.popup.hasAttribute('data-sc-style-popup')).toBe(true);
    expect(f.errors).not.toHaveBeenCalled();
  } finally {
    f.host.dispose();
  }
});

test.each(['replaced', 'removed'] as const)(
  'releases a %s native relationship and leaves independent model pickers untouched',
  async (change) => {
    const f = fixture(false);
    try {
      f.trigger.click();
      await settle(f.frames);
      expect(f.popup.hasAttribute('data-sc-style-theme-root')).toBe(true);
      const model = document.createElement('div');
      model.className = 'sc-model-popup';
      const choices = document.createElement('div');
      choices.id = 'model-choices';
      choices.setAttribute('role', 'listbox');
      model.append(choices);
      document.body.append(model);
      if (change === 'removed') f.trigger.removeAttribute('aria-controls');
      else f.trigger.setAttribute('aria-controls', choices.id);
      await settle(f.frames);
      expect(f.popup.hasAttribute('data-sc-style-popup')).toBe(false);
      expect(f.popup.hasAttribute('data-sc-menu-surface')).toBe(false);
      expect(f.popup.hasAttribute('data-sc-style-theme-root')).toBe(false);
      expect(f.search.hasAttribute('data-sc-inline-search')).toBe(false);
      expect(choices.hasAttribute('data-sc-menu-surface')).toBe(false);
      expect(f.present).not.toHaveBeenCalled();
      expect(f.errors).not.toHaveBeenCalled();
    } finally {
      f.host.dispose();
    }
  },
);

test('reports an unexpected native presentation failure and releases the popup annotations', async () => {
  const f = fixture(false);
  const failure = new Error('Host search geometry unavailable');
  jest.spyOn(NativeInlineComboSearch.prototype, 'present').mockImplementation(() => {
    throw failure;
  });
  try {
    f.trigger.click();
    await settle(f.frames);
    expect(f.errors).toHaveBeenCalledTimes(1);
    expect(f.errors).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'Native dropdown presentation failed.', cause: failure }),
    );
    expect(f.popup.hasAttribute('data-sc-style-popup')).toBe(false);
    expect(f.popup.hasAttribute('data-sc-menu-surface')).toBe(false);
    expect(f.popup.hasAttribute('data-sc-style-theme-root')).toBe(false);
  } finally {
    f.host.dispose();
  }
});

test('does not associate an empty native control reference with an unrelated anonymous popup', async () => {
  const f = fixture(false);
  try {
    f.trigger.setAttribute('aria-controls', '');
    const popup = document.createElement('div');
    popup.setAttribute('role', 'listbox');
    document.body.append(popup);
    await settle(f.frames);
    expect(popup.hasAttribute('data-sc-style-popup')).toBe(false);
    expect(popup.hasAttribute('data-sc-menu-surface')).toBe(false);
    expect(f.present).not.toHaveBeenCalled();
    expect(f.errors).not.toHaveBeenCalled();
  } finally {
    f.host.dispose();
  }
});
