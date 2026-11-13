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
/** Prove directional noodle reveal, continuous reroutes, interruption, and native-paint cleanup. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WorkflowMasonryLinkMotion } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryLinkMotion.js';
import type { NativeLinkPathStroke } from '../../../frontend/comfyui/ui/surface/masonry/NativeLinkPathRecording.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Create two native segments with a border sharing the first trajectory. */
function fixture() {
  const container = document.createElement('div');
  document.body.append(container);
  const nodeLayer = document.createElement('div');
  nodeLayer.className = 'ph-no-capture';
  container.append(nodeLayer);
  const create = document.createElementNS.bind(document);
  jest.spyOn(document, 'createElementNS').mockImplementation((namespace, name, options) => {
    const element = create(namespace, name, options);
    if (name === 'path')
      Object.defineProperty(element, 'getTotalLength', {
        value: () => (element.getAttribute('d') === 'M0 0 L100 0' ? 100 : 200),
      });
    return element;
  });
  const stroke: NativeLinkPathStroke = {
    link: 'one',
    path: 'M0 0 L100 0',
    transform: 'matrix(1 0 0 1 0 0)',
    color: '#abc',
    width: 2,
    opacity: 1,
    lineCap: 'round',
  };
  const restore = jest.fn();
  const motion = new WorkflowMasonryLinkMotion(container, {
    strokes: [
      { ...stroke, width: 4 },
      stroke,
      { ...stroke, path: 'M100 0 C100 100 200 100 200 0' },
    ],
    restore,
  });
  return { container, nodeLayer, restore, motion };
}

test('draws along whole rerouted noodles in graph direction and retracts from the input', () => {
  const { container, nodeLayer, motion, restore } = fixture();
  const exiting = motion.frames(false, false);
  expect(exiting).toHaveLength(3);
  expect(exiting[0]?.frames.map((frame) => frame.offset)).toEqual([0, 0, 1 / 3, 1]);
  expect(exiting[2]?.frames.map((frame) => frame.offset)).toEqual([0, 1 / 3, 1, 1]);
  expect(exiting[0]?.frames.map((frame) => frame.strokeDashoffset)).toEqual([1, 1, 0, 0]);
  const entering = motion.frames(true, false);
  expect(entering[0]?.frames[1]?.offset).toBeCloseTo(2 / 3);
  expect(entering[2]?.frames[2]?.offset).toBeCloseTo(2 / 3);
  expect(entering[2]?.frames.map((frame) => frame.strokeDashoffset)).toEqual([0, 0, 1, 1]);
  expect(container.querySelectorAll('path')).toHaveLength(3);
  expect(container.querySelector('path')?.getAttribute('d')).toBe('M0 0 L100 0');
  expect(container.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
  expect(container.querySelector('svg')?.nextElementSibling).toBe(nodeLayer);
  expect(container.querySelector('svg')?.style.zIndex).toBe('0');
  motion.dispose();
  expect(restore).toHaveBeenCalledTimes(1);
  expect([...container.children]).toEqual([nodeLayer]);
});

test('reverses at the current stroke tip and keeps remaining reroutes sequential', () => {
  const { motion } = fixture();
  const first = motion.frames(true, false);
  for (const [index, effect] of first.entries())
    effect.element.style.strokeDashoffset = index < 2 ? '0' : '0.5';
  const reversing = motion.frames(false, true);
  expect(reversing[0]?.frames.map((frame) => frame.strokeDashoffset)).toEqual([0, 0, 0, 0]);
  expect(reversing[2]?.frames[0]?.strokeDashoffset).toBe(0.5);
  expect(reversing[2]?.frames[1]?.offset).toBe(0);
  expect(reversing[2]?.frames[2]?.offset).toBe(1);
  motion.dispose();
});

test('removes temporary vectors even if native restoration fails', () => {
  const { container, motion, restore } = fixture();
  restore.mockImplementation(() => {
    throw new Error('Renderer stopped');
  });
  expect(() => motion.dispose()).toThrow('Renderer stopped');
  expect(container.querySelector('svg')).toBeNull();
});

test('holds a replug during reversal and releases animation ownership on disposal', async () => {
  const { container, motion, restore } = fixture();
  const pending: Array<{ frames: Keyframe[]; finish(): void; cancel: ReturnType<typeof jest.fn> }> =
    [];
  for (const element of container.querySelectorAll('path'))
    Object.defineProperty(element, 'animate', {
      value: (frames: Keyframe[], options: KeyframeAnimationOptions) => {
        expect(options.duration).toBe(100);
        let finish = (): void => undefined;
        let reject = (_error: Error): void => undefined;
        const finished = new Promise<void>((resolve, failure) => {
          finish = resolve;
          reject = failure;
        });
        const effect = {
          frames,
          finished,
          finish,
          cancel: jest.fn(() => reject(new Error('Cancelled'))),
        };
        pending.push(effect);
        return effect;
      },
    });
  motion.hold(true);
  expect(container.querySelector('svg')?.style.visibility).toBe('hidden');
  const replug = motion.run(false, false);
  expect(container.querySelector('svg')?.style.visibility).toBe('visible');
  expect(pending).toHaveLength(3);
  expect(pending[0]?.frames[0]?.strokeDashoffset).toBe(1);
  motion.hold(true);
  await replug;
  expect(pending.every((effect) => effect.cancel.mock.calls.length === 1)).toBe(true);
  expect(
    [...container.querySelectorAll('path')].every((path) => path.style.strokeDashoffset === '1'),
  ).toBe(true);
  const unplug = motion.run(true, false);
  motion.dispose();
  await unplug;
  expect(container.querySelector('svg')).toBeNull();
  expect(restore).toHaveBeenCalledTimes(1);
});

test('honors reduced motion without creating compositor effects', async () => {
  const { container, motion } = fixture();
  const animate = jest.fn();
  for (const element of container.querySelectorAll('path'))
    Object.defineProperty(element, 'animate', { value: animate });
  motion.hold(true);
  await motion.run(false, true);
  expect(animate).not.toHaveBeenCalled();
  expect(
    [...container.querySelectorAll('path')].every((path) => path.style.strokeDashoffset === '0'),
  ).toBe(true);
  motion.dispose();
});
