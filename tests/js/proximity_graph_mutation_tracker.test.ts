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
/**
 * Verify bounded proximity refreshes at LiteGraph mutation boundaries.
 */

import { jest } from '@jest/globals';
import type { ComfyGraph } from '../../frontend/comfyui/ui/types/graph.js';
import { ProximityGraphMutationTracker } from '../../frontend/comfyui/ui/overlays/proximity/ProximityGraphMutationTracker.js';

interface MutableTestGraph extends ComfyGraph {
  onNodeAdded?: (this: ComfyGraph, value: unknown) => unknown;
  onNodeRemoved?: (this: ComfyGraph, value: unknown) => unknown;
  onConnectionChange?: (this: ComfyGraph, value: unknown) => unknown;
  onNodeConnectionChange?: (this: ComfyGraph, value: unknown) => unknown;
  removeLink?: (this: ComfyGraph, value: unknown) => unknown;
}

describe('ProximityGraphMutationTracker', () => {
  function createHarness(graph: MutableTestGraph): {
    tracker: ProximityGraphMutationTracker;
    schedulePreview: jest.Mock;
    frames: FrameRequestCallback[];
  } {
    const frames: FrameRequestCallback[] = [];
    const schedulePreview = jest.fn();
    const tracker = new ProximityGraphMutationTracker(
      {
        isProximityEnabled: () => true,
        schedulePreview,
      },
      {
        raf: (callback) => {
          frames.push(callback);
          return frames.length;
        },
      },
    );
    tracker.attach(graph);
    return { tracker, schedulePreview, frames };
  }

  test('creates inventory callbacks and refreshes once per mutation', () => {
    const graph: MutableTestGraph = {};
    const { schedulePreview } = createHarness(graph);

    graph.onNodeAdded?.({});
    graph.onNodeRemoved?.({});

    expect(schedulePreview).toHaveBeenCalledTimes(2);
    expect(schedulePreview).toHaveBeenNthCalledWith(1, { graph });
    expect(schedulePreview).toHaveBeenNthCalledWith(2, { graph });
  });

  test('preserves callback context, arguments, and return values', () => {
    const graph: MutableTestGraph = {};
    const original = jest.fn(function callback(this: ComfyGraph, value: unknown) {
      expect(this).toBe(graph);
      return value;
    });
    graph.onNodeAdded = original;
    createHarness(graph);
    const marker = {};

    const result = graph.onNodeAdded?.(marker);

    expect(original).toHaveBeenCalledWith(marker);
    expect(result).toBe(marker);
  });

  test('coalesces ownership callbacks until two graph-settling frames pass', () => {
    const graph: MutableTestGraph = {};
    const { schedulePreview, frames } = createHarness(graph);

    graph.onConnectionChange?.({});
    graph.onNodeConnectionChange?.({});

    expect(schedulePreview).not.toHaveBeenCalled();
    frames.splice(0).forEach((callback) => callback(0));
    expect(schedulePreview).not.toHaveBeenCalled();
    frames.splice(0).forEach((callback) => callback(16));
    expect(schedulePreview).toHaveBeenCalledTimes(1);
    expect(schedulePreview).toHaveBeenCalledWith({ graph });
  });

  test('observes removeLink while preserving its contract', () => {
    const graph: MutableTestGraph = {};
    const removed = {};
    const original = jest.fn(function removeLink(this: ComfyGraph, value: unknown) {
      expect(this).toBe(graph);
      return value === removed;
    });
    graph.removeLink = original;
    const { schedulePreview, frames } = createHarness(graph);

    const result = graph.removeLink?.(removed);

    expect(result).toBe(true);
    expect(original).toHaveBeenCalledWith(removed);
    expect(schedulePreview).not.toHaveBeenCalled();
    frames.splice(0).forEach((callback) => callback(0));
    frames.splice(0).forEach((callback) => callback(16));
    expect(schedulePreview).toHaveBeenCalledTimes(1);
  });

  test('attaches only once to a graph', () => {
    const graph: MutableTestGraph = {};
    const { tracker, schedulePreview } = createHarness(graph);
    const callback = graph.onNodeAdded;

    tracker.attach(graph);
    graph.onNodeAdded?.({});

    expect(graph.onNodeAdded).toBe(callback);
    expect(schedulePreview).toHaveBeenCalledTimes(1);
  });

  test('does not schedule work while proximity is disabled', () => {
    const graph: MutableTestGraph = {};
    const schedulePreview = jest.fn();
    const tracker = new ProximityGraphMutationTracker(
      {
        isProximityEnabled: () => false,
        schedulePreview,
      },
      null,
    );
    tracker.attach(graph);

    graph.onNodeAdded?.({});
    graph.onConnectionChange?.({});

    expect(schedulePreview).not.toHaveBeenCalled();
  });
});
