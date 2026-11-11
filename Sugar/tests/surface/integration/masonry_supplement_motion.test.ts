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
/** Prove owned presentation cards use native masonry assembly, reversal and reduced-motion behavior. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  identifyMasonrySupplement,
  releaseMasonrySupplement,
} from '../../../frontend/comfyui/ui/surface/masonry/MasonrySupplementBoundary.js';
import { WorkflowMasonryView } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryView.js';
import { measureWorkflowMasonryGraph } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryGraphGeometry.js';
import { WorkflowMasonryTransition } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryTransition.js';
import { projectWorkflowMasonry } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import { CubeFaceCardStateService } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('an owned card travels between its retained native frame and its first masonry slot', async () => {
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const native = document.createElement('div');
  native.className = 'lg-node';
  native.dataset.nodeId = '55';
  native.style.width = '400px';
  const source = document.createElement('div');
  native.append(source);
  root.append(native);
  document.body.append(root);
  const view = new WorkflowMasonryView({
    document,
    sections: [],
    cardState: new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} }),
    openCube() {},
  });
  root.append(view.backdrop, view.element);
  const card = document.createElement('article');
  const input = document.createElement('input');
  input.value = '42';
  card.append(input);
  view.element.querySelector('.sc-plugin-overrides-slot')?.append(card);
  identifyMasonrySupplement(card, source, 'plugin:global-overrides');
  jest.spyOn(root, 'getBoundingClientRect').mockReturnValue(rectangle(0, 0, 1200, 800));
  jest.spyOn(native, 'getBoundingClientRect').mockReturnValue(rectangle(200, 250, 200, 90));
  jest.spyOn(card, 'getBoundingClientRect').mockReturnValue(rectangle(30, 70, 400, 160));
  const frames: Array<{
    element: HTMLElement;
    frames: Keyframe[];
    options: KeyframeAnimationOptions;
  }> = [];
  for (const element of [native, card, view.backdrop])
    Object.defineProperty(element, 'animate', {
      configurable: true,
      value: (keyframes: Keyframe[], options: KeyframeAnimationOptions) => {
        frames.push({ element, frames: keyframes, options });
        return { finished: Promise.resolve(), cancel: jest.fn() };
      },
    });
  expect(view.transitionCards()).toEqual([{ id: 'plugin:global-overrides', cell: card }]);
  const measureGraph = () => measureWorkflowMasonryGraph(root, []);
  expect(measureGraph().cards.get('plugin:global-overrides')?.element).toBe(native);
  const transition = new WorkflowMasonryTransition();
  const options = {
    surface: view.element,
    backdrop: view.backdrop,
    cards: view.transitionCards(),
    measureGraph,
  };
  await transition.run(options, true, false);
  const entering = frames.find((effect) => effect.element === card);
  expect(entering?.frames[0]?.transform).toBe('translate(170px, 180px) scale(0.5)');
  expect(entering?.frames[1]?.transform).toBe('translate(0px, 0px) scale(1)');
  expect(frames.find((effect) => effect.element === native)?.options.duration).toBe(
    entering?.options.duration,
  );
  expect(card.querySelector('input')).toBe(input);
  expect(input.value).toBe('42');
  expect(native.dataset.nodeId).toBe('55');
  frames.length = 0;
  await transition.run(options, false, false);
  expect(frames.find((effect) => effect.element === card)?.frames[1]?.transform).toBe(
    'translate(170px, 180px) scale(0.5)',
  );
  frames.length = 0;
  await transition.run(options, true, true);
  expect(frames).toEqual([]);
  releaseMasonrySupplement(card, source);
  expect(measureGraph().cards.size).toBe(0);
  view.dispose();
});

test('presentation-only native nodes do not become duplicate workflow sections', () => {
  expect(
    projectWorkflowMasonry({ _nodes: [{ id: 55, sugarcubesPresentationOnly: true }] }, []),
  ).toEqual([]);
});

/** Supply native screen geometry at the deterministic browser boundary. */
function rectangle(x: number, y: number, width: number, height: number): DOMRect {
  return {
    x,
    y,
    width,
    height,
    left: x,
    top: y,
    right: x + width,
    bottom: y + height,
    toJSON: () => ({}),
  };
}
