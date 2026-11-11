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
/** Prove current native choices are prepared without losing gestures, focus or popup identity. */
import { jest } from '@jest/globals';
import { NativeChoicePreparation } from '../../../frontend/comfyui/ui/surface/NativeChoicePreparation.js';

/** Model only the native renderer's existing popup trigger. */
function fixture() {
  const root = document.createElement('div');
  const trigger = document.createElement('button');
  trigger.setAttribute('role', 'combobox');
  trigger.setAttribute('aria-expanded', 'false');
  root.append(trigger);
  document.body.append(root);
  let finish: ((ready: boolean) => void) | undefined;
  const prepare = jest.fn(
    () =>
      new Promise<boolean>((resolve) => {
        finish = resolve;
      }),
  );
  const nativeOpen = jest.fn(() => {
    trigger.setAttribute('aria-expanded', 'true');
  });
  trigger.addEventListener('click', nativeOpen);
  const owner = new NativeChoicePreparation(root, prepare);
  return { root, trigger, prepare, nativeOpen, owner, finish: (ready = true) => finish?.(ready) };
}

/** Settle gesture admission and the renderer's deferred opening. */
async function settled(): Promise<void> {
  for (let index = 0; index < 12; index++) await Promise.resolve();
}

afterEach(() => {
  document.body.replaceChildren();
});

test('coalesces pointer intent and click, then opens the same native trigger with fresh choices', async () => {
  const f = fixture();
  f.trigger.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }));
  f.trigger.click();
  f.trigger.click();
  await settled();
  expect(f.prepare).toHaveBeenCalledTimes(1);
  expect(f.nativeOpen).not.toHaveBeenCalled();
  expect(f.trigger.getAttribute('aria-busy')).toBe('true');
  f.finish();
  await settled();
  expect(f.nativeOpen).toHaveBeenCalledTimes(1);
  expect(document.activeElement).toBe(f.trigger);
  expect(f.trigger.hasAttribute('aria-busy')).toBe(false);
  f.owner.dispose();
});

test.each(['Enter', ' ', 'ArrowDown', 'ArrowUp', 'F4'])(
  'prepares keyboard opening through the native renderer: %s',
  async (key) => {
    const f = fixture();
    f.trigger.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    await settled();
    f.finish();
    await settled();
    expect(f.prepare).toHaveBeenCalledTimes(1);
    expect(f.nativeOpen).toHaveBeenCalledTimes(1);
    f.owner.dispose();
  },
);

test.each(['outside', 'escape', 'dispose', 'failure'])(
  'does not open a stale or dismissed popup after preparation: %s',
  async (reason) => {
    const f = fixture();
    f.trigger.click();
    await settled();
    if (reason === 'outside')
      document.body.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
    if (reason === 'escape')
      f.trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    if (reason === 'dispose') f.owner.dispose();
    f.finish(reason !== 'failure');
    await settled();
    expect(f.nativeOpen).not.toHaveBeenCalled();
    f.owner.dispose();
  },
);
