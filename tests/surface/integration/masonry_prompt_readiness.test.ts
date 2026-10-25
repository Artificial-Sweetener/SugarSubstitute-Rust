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
/** Require completed prompt construction before native masonry mount can be presented. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { registerPromptEditorSurfaceOwner } from '../../../frontend/comfyui/ui/prompt_editor/PromptEditorSurface.js';
import { WorkflowMasonryView } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryView.js';
import { projectWorkflowMasonry } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import { CubeFaceCardStateService } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Mount one native field through the production masonry lifecycle. */
function masonry(): WorkflowMasonryView {
  const view = new WorkflowMasonryView({
    document,
    sections: projectWorkflowMasonry(
      { _nodes: [{ id: 1, type: 'StringConstant', widgets: [{ name: 'text', value: 'prompt' }] }] },
      [],
    ),
    cardState: new CubeFaceCardStateService({ nodes: new CubeNodeCatalog(), history: {} }),
    openCube() {},
  });
  document.body.append(view.element);
  return view;
}

test('finishes component construction before committing masonry geometry or exposing its contents', async () => {
  const view = masonry();
  let complete = (): void => undefined;
  let arrived = (): void => undefined;
  const constructing = new Promise<void>((resolve) => {
    complete = resolve;
  });
  const requested = new Promise<void>((resolve) => {
    arrived = resolve;
  });
  const release = registerPromptEditorSurfaceOwner(document, {
    mount: () => () => undefined,
    refresh: () => true,
    ready: async (root) => {
      expect(root).toBe(view.element);
      expect(root.querySelector('textarea')).not.toBeNull();
      arrived();
      await constructing;
    },
    dispose: () => undefined,
  });
  const layout = jest.spyOn(view, 'layout');
  let finished = false;
  try {
    const mounted = view
      .mountCards({
        mount: (target) => {
          target.append(document.createElement('textarea'));
          return { refresh() {}, unmount() {} };
        },
        dispose() {},
      })
      .then(() => {
        finished = true;
      });
    await requested;
    expect(finished).toBe(false);
    expect(layout).not.toHaveBeenCalled();
    expect(view.element.hasAttribute('data-sc-masonry-preparing')).toBe(true);
    complete();
    await mounted;
    expect(finished).toBe(true);
    expect(layout).not.toHaveBeenCalled();
    view.setTransitioning(true);
    expect(layout).toHaveBeenCalledTimes(1);
  } finally {
    complete();
    release();
    view.dispose();
  }
});

test('reports failed component construction without admitting an incomplete presentation', async () => {
  const view = masonry();
  const failure = new Error('Prompt construction failed');
  const release = registerPromptEditorSurfaceOwner(document, {
    mount: () => () => undefined,
    refresh: () => true,
    ready: async () => {
      throw failure;
    },
    dispose: () => undefined,
  });
  try {
    await expect(
      view.mountCards({
        mount: () => ({ refresh() {}, unmount() {} }),
        dispose() {},
      }),
    ).rejects.toBe(failure);
    expect(view.element.hasAttribute('data-sc-masonry-preparing')).toBe(true);
  } finally {
    release();
    view.dispose();
  }
});
