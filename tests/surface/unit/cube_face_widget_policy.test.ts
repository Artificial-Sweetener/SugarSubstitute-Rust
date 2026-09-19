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
/** Verify effective Cube-face widget ownership independently from native rendering. */

import { describe, expect, jest, test } from '@jest/globals';

import {
  createCubeFaceNodeData,
  withCubeFaceNodePresentation,
} from '../../../frontend/comfyui/ui/surface/CubeFaceNodePresentationPolicy.js';
import {
  cubeFaceUnconsumedWidgets,
  cubeFaceVisibleWidgets,
  isCubeFaceWidgetConsumed,
} from '../../../frontend/comfyui/ui/surface/CubeFaceWidgetPolicy.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

describe('CubeFaceWidgetPolicy', () => {
  test('excludes widgets supplied through singular or plural input links', () => {
    const prompt = { name: 'text', value: 'stale prompt' };
    const style = { name: 'style', value: 'cinematic' };
    const seed = { name: 'seed', value: 1 };
    const node: ComfyNode = {
      widgets: [prompt, style, seed],
      inputs: [
        { name: 'text', link: 0, widget: { name: 'text' } },
        { name: 'style', links: ['style-link'], widget: { name: 'style' } },
        { name: 'seed', link: null, widget: { name: 'seed' } },
      ],
    };

    expect(isCubeFaceWidgetConsumed(node, prompt)).toBe(true);
    expect(isCubeFaceWidgetConsumed(node, style)).toBe(true);
    expect(isCubeFaceWidgetConsumed(node, seed)).toBe(false);
    expect(cubeFaceUnconsumedWidgets(node)).toEqual([seed]);
    expect(cubeFaceVisibleWidgets(node)).toEqual([seed]);
  });

  test('uses adjacent widget identity and retains unrelated or host-hidden controls correctly', () => {
    const prompt = { name: 'text' };
    const hidden = { name: 'advanced' };
    const node: ComfyNode = {
      widgets: [prompt, hidden],
      inputs: [{ name: 'text', link: 12 }],
      isWidgetVisible: jest.fn((widget) => widget !== hidden),
    };

    expect(cubeFaceUnconsumedWidgets(node)).toEqual([prompt, hidden]);
    expect(cubeFaceVisibleWidgets(node)).toEqual([prompt]);
  });

  test('projects only effective widgets into Vue data without mutating graph ownership', () => {
    const consumed = { name: 'text', value: 'stale prompt' };
    const editable = { name: 'strength', value: 0.7 };
    const widgets = [consumed, editable];
    const node: ComfyNode = {
      widgets,
      inputs: [{ name: 'text', link: 9, widget: { name: 'text' } }],
    };

    const projected = createCubeFaceNodeData(
      { widgets, inputs: node.inputs, outputs: [{ name: 'CONDITIONING' }] },
      node,
    );

    expect(projected.widgets).toEqual([editable]);
    expect(projected.inputs).toEqual([]);
    expect(projected.outputs).toEqual([]);
    expect(node.widgets).toBe(widgets);
  });

  test('temporarily masks consumed widgets during native operations and restores exact state', () => {
    const consumed = { name: 'text' };
    const editable = { name: 'strength' };
    const widgets = [consumed, editable];
    const node: ComfyNode = {
      widgets,
      inputs: [{ name: 'text', link: 4, widget: { name: 'text' } }],
    };

    withCubeFaceNodePresentation(node, () => {
      expect(node.widgets).toEqual([editable]);
    });

    expect(node.widgets).toBe(widgets);
  });
});
