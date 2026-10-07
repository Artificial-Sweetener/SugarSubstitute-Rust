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
/** Prove independent background fades, reversal continuity, and cancellation ownership. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WorkflowMasonryTransition } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryTransition.js';
import { fluentMotion } from '../../../frontend/comfyui/ui/surface/appearance/FluentThemeRuntime.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('fades the background in both directions without replacing native card layout', async () => {
  const setup = fixture();
  const transition = new WorkflowMasonryTransition();
  const geometry = jest.spyOn(setup.card, 'getBoundingClientRect');
  const entry = transition.run(setup.view, true, false);
  expect(setup.frames).toEqual([[{ opacity: 0 }, { opacity: 1 }]]);
  expect(setup.options[0]).toMatchObject({
    duration: fluentMotion.duration,
    easing: fluentMotion.easing,
    fill: 'both',
  });
  expect(setup.surface.style.opacity).toBe('1');
  expect(setup.surface.inert).toBe(true);
  setup.effects[0]?.finish();
  await entry;
  expect(setup.surface.inert).toBeUndefined();
  const exit = transition.run(setup.view, false, false);
  expect(setup.frames[1]).toEqual([{ opacity: 1 }, { opacity: 0 }]);
  expect(setup.backdrop.style.opacity).toBe('0');
  setup.effects[1]?.finish();
  await exit;
  expect(setup.effects.every((effect) => effect.cancel.mock.calls.length === 1)).toBe(true);
  expect(geometry).not.toHaveBeenCalled();
  expect(setup.card.style.transform).toBe('translate(12px, 80px)');
});

test('reduced motion switches immediately and cancels any pending fade', async () => {
  const setup = fixture();
  const transition = new WorkflowMasonryTransition();
  const pending = transition.run(setup.view, true, false);
  await transition.run(setup.view, false, true);
  await pending;
  expect(setup.effects).toHaveLength(1);
  expect(setup.effects[0]?.cancel).toHaveBeenCalledTimes(1);
  expect(setup.backdrop.style.opacity).toBe('0');
  await transition.run(setup.view, true, true);
  expect(setup.backdrop.style.opacity).toBe('1');
  expect(setup.effects).toHaveLength(1);
});

test('reverses from displayed opacity and old completion cannot cancel the new fade', async () => {
  const setup = fixture();
  const transition = new WorkflowMasonryTransition();
  const entry = transition.run(setup.view, true, false);
  setup.backdrop.style.opacity = '0.55';
  const exit = transition.run(setup.view, false, false);
  await entry;
  expect(setup.frames[1]).toEqual([{ opacity: '0.55' }, { opacity: 0 }]);
  expect(setup.effects[1]?.cancel).not.toHaveBeenCalled();
  transition.cancel();
  await exit;
  expect(setup.effects[1]?.cancel).toHaveBeenCalledTimes(1);
});

/** Model the external WAAPI completion boundary without mocking transition policy. */
function fixture() {
  const surface = document.createElement('section');
  const backdrop = document.createElement('div');
  surface.style.opacity = '0';
  const card = document.createElement('article');
  card.style.transform = 'translate(12px, 80px)';
  surface.append(card);
  document.body.append(backdrop, surface);
  const effects: Array<{ cancel: ReturnType<typeof jest.fn>; finish(): void }> = [];
  const frames: Keyframe[][] = [];
  const options: KeyframeAnimationOptions[] = [];
  Object.defineProperty(backdrop, 'animate', {
    value: (keyframes: Keyframe[], timing: KeyframeAnimationOptions) => {
      const completion: { resolve?: () => void; reject?: (reason: Error) => void } = {};
      const finished = new Promise<void>((resolve, reject) => {
        completion.resolve = resolve;
        completion.reject = reject;
      });
      const effect = {
        finished,
        finish: () => completion.resolve?.(),
        cancel: jest.fn(() => completion.reject?.(new Error('Cancelled'))),
      };
      effects.push(effect);
      frames.push(keyframes);
      options.push(timing);
      return effect;
    },
  });
  const view = {
    surface,
    backdrop,
    cards: [],
    graph: { cards: new Map(), layers: [], viewport: surface.getBoundingClientRect() },
  };
  return { surface, backdrop, card, effects, frames, options, view };
}
