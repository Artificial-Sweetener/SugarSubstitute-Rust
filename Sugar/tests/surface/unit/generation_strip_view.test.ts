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
/** Prove stable generation-strip controls, batch edits and action feedback. */
import { jest } from '@jest/globals';
import { GenerationStripView } from '../../../frontend/comfyui/ui/surface/frame/GenerationStripView.js';
import { generationStripCss } from '../../../frontend/comfyui/ui/surface/frame/GenerationStripStyles.js';

afterEach(() => document.body.replaceChildren());

/** Build real strip DOM around a deterministic external generation boundary. */
function fixture() {
  const actions = {
    setBatch: jest.fn<(value: number) => void>(),
    setMode: jest.fn<(mode: 'generate' | 'continuous') => void>(),
    generate: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
    cancel: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
    skip: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
    report: jest.fn<(error: unknown) => void>(),
  };
  const view = new GenerationStripView(document, actions);
  document.body.append(view.element);
  const state = {
    batch: 3,
    generateEnabled: true,
    skipEnabled: true,
    cancelEnabled: true,
    mode: 'generate' as const,
    looping: false,
  };
  view.update(state);
  const input = view.element.querySelector('input')!;
  return { actions, view, state, input };
}

test('source order and control identity survive native execution state changes', () => {
  const { view, state } = fixture();
  const controls = [...view.element.querySelectorAll('.sc-generation-action')];
  expect(controls.map((control) => control.getAttribute('data-action'))).toEqual([
    'cancel',
    'generate',
    'skip',
  ]);
  view.update({ ...state, skipEnabled: false, cancelEnabled: false });
  expect([...view.element.querySelectorAll('.sc-generation-action')]).toEqual(controls);
  expect(view.element.querySelector<HTMLButtonElement>('[data-action="skip"]')!.disabled).toBe(
    true,
  );
  view.dispose();
  expect(document.querySelector('.sc-generation-strip')).toBeNull();
});

test('direct entry, steps and Escape edit cancellation use the host batch setting', () => {
  const { actions, view, input, state } = fixture();
  input.value = '5';
  input.dispatchEvent(new Event('change'));
  expect(actions.setBatch).toHaveBeenLastCalledWith(5);
  view.update({ ...state, batch: 5 });
  view.element.querySelector<HTMLButtonElement>('[data-direction="up"]')!.click();
  expect(actions.setBatch).toHaveBeenLastCalledWith(6);
  input.focus();
  input.value = '77';
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
  expect(actions.setBatch).not.toHaveBeenCalledWith(77);
  input.value = '<invalid>';
  input.dispatchEvent(new Event('change'));
  expect(input.value).toBe('5');
});

test('preview updates preserve an in-progress batch edit and unrelated focus', () => {
  const { view, input, state } = fixture();
  input.focus();
  input.value = '12';
  view.update({ ...state, skipEnabled: false });
  expect(input.value).toBe('12');
  expect(document.activeElement).toBe(input);
  view.update({ ...state, batch: 7 });
  expect(input.value).toBe('7');
});

test('each action delegates once and failure reaches the existing feedback boundary', async () => {
  const { actions, view } = fixture();
  for (const id of ['generate', 'cancel', 'skip'])
    view.element.querySelector<HTMLButtonElement>(`[data-action="${id}"]`)!.click();
  expect(actions.generate).toHaveBeenCalledTimes(1);
  expect(actions.cancel).toHaveBeenCalledTimes(1);
  expect(actions.skip).toHaveBeenCalledTimes(1);
  const error = new Error('host rejected generation');
  actions.generate.mockRejectedValue(error);
  view.element.querySelector<HTMLButtonElement>('[data-action="generate"]')!.click();
  await Promise.resolve();
  expect(actions.report).toHaveBeenCalledWith(error);
});

test('right-clicking Generate chooses its mode, and Generate shows the chosen one', () => {
  const { actions, view, state } = fixture();
  const generate = view.element.querySelector<HTMLButtonElement>('[data-action="generate"]')!;
  generate.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  const rows = [
    ...document.querySelectorAll<HTMLElement>('[role="menuitemradio"],[role="menuitem"]'),
  ];
  const labels = rows.map((row) => row.textContent?.trim());
  expect(labels).toEqual(expect.arrayContaining(['Generate', 'Continuous']));
  rows.find((row) => row.textContent?.trim() === 'Continuous')!.click();
  expect(actions.setMode).toHaveBeenCalledWith('continuous');
  view.update({ ...state, mode: 'continuous' });
  expect(generate.dataset.mode).toBe('continuous');
  expect(generate.getAttribute('aria-label')).toBe('Generate continuously');
  expect(
    generate
      .querySelector<HTMLElement>('.sc-generation-glyph')!
      .style.getPropertyValue('--sc-generation-glyph'),
  ).toContain('infinity.svg');
  // While the loop runs, Generate stops it, even when a new run could not be queued.
  view.update({ ...state, mode: 'continuous', looping: true, generateEnabled: false });
  expect(generate.disabled).toBe(false);
  expect(generate.getAttribute('aria-label')).toBe('Stop continuous generation');
  view.dispose();
});

test('glyphs keep the Python sizes: Generate 16 px, Continuous 24 px, Skip 20 px', () => {
  const css = generationStripCss.replace(/\s+/g, ' ');
  expect(css).toContain('.sc-generation-glyph { display:block; width:16px; height:16px;');
  expect(css).toContain(
    ".sc-generation-action[data-action='generate'][data-mode='continuous'] .sc-generation-glyph { width:24px; height:24px; }",
  );
  expect(css).toContain(
    ".sc-generation-action[data-action='skip'] .sc-generation-glyph { width:20px; height:20px; }",
  );
});
