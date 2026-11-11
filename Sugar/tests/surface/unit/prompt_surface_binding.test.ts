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
/** Prove promoted prompts mount against their boundary owner and ignore unrelated controls. */
import {
  mountPromptEditorSurface,
  registerPromptEditorSurfaceOwner,
  type PromptEditorSurface,
} from '../../../frontend/comfyui/ui/prompt_editor/PromptEditorSurface.js';
import { CubePromptTextareaScope } from '../../../frontend/comfyui/ui/surface/CubePromptTextareaScope.js';
import { CubePromptDomWidgetSizingHost } from '../../../frontend/comfyui/ui/surface/CubePromptDomWidgetSizingHost.js';
import {
  setCubePromptWidgetHeight,
  cubePromptWidgetHeight,
  clearCubePromptWidgetHeight,
} from '../../../frontend/comfyui/ui/surface/CubePromptWidgetHeightStore.js';
import type { ComfyNode, ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';

test('Nodes 1 presentation release retains the rich session allocation across renderer switches', () => {
  const release = registerPromptEditorSurfaceOwner(document, {
    mount: () => () => undefined,
    refresh: () => true,
    ready: async () => undefined,
    dispose: () => undefined,
  });
  const widget: ComfyWidget = { name: 'text', value: 'prompt' };
  const textarea = document.createElement('textarea');
  const sizing = new CubePromptDomWidgetSizingHost(
    document,
    () => undefined,
    () => false,
  );
  try {
    sizing.reconcile(textarea, widget, { widgets: [widget] });
    setCubePromptWidgetHeight(widget, 148);
    sizing.release(textarea);
    expect(cubePromptWidgetHeight(widget)).toBe(148);
  } finally {
    sizing.dispose();
    release();
    clearCubePromptWidgetHeight(widget);
  }
});

test('extension setup replacement retires the previous owner and fences its late registration release', () => {
  let retired = 0;
  let mounts = 0;
  const first = registerPromptEditorSurfaceOwner(document, {
    mount: () => () => undefined,
    refresh: () => true,
    ready: async () => undefined,
    dispose: () => {
      retired++;
    },
  });
  const second = registerPromptEditorSurfaceOwner(document, {
    mount: () => {
      mounts++;
      return () => undefined;
    },
    refresh: () => true,
    ready: async () => undefined,
    dispose: () => undefined,
  });
  try {
    first();
    expect(retired).toBe(1);
    mountPromptEditorSurface({
      node: {},
      widget: { name: 'text' },
      textarea: document.createElement('textarea'),
      geometryChanged: () => undefined,
      hostReadOnly: () => false,
    })?.();
    expect(mounts).toBe(1);
  } finally {
    first();
    second();
  }
});

test('promoted prompt binds the boundary widget while preserving its internal presentation', () => {
  const internal: ComfyWidget = { name: 'text', type: 'customtext', value: 'stale' };
  const boundary: ComfyWidget = { name: 'prompt input', type: 'customtext', value: 'current' };
  const node: ComfyNode = {
    title: 'Positive prompt',
    widgets: [internal],
    inputs: [{ name: 'text', type: 'STRING', link: 1, widget: { name: 'text' } }],
  };
  const owner: ComfyNode = { widgets: [boundary] };
  const mounted: PromptEditorSurface[] = [];
  let releases = 0;
  const unregister = registerPromptEditorSurfaceOwner(document, {
    mount: (surface) => {
      mounted.push(surface);
      return () => {
        releases++;
      };
    },
    refresh: () => true,
    ready: async () => undefined,
    dispose: () => undefined,
  });
  const root = document.createElement('section');
  const textarea = document.createElement('textarea');
  root.append(textarea);
  const scope = new CubePromptTextareaScope(root);
  try {
    const sources = [{ node: owner, name: boundary.name, targetName: internal.name }];
    scope.reconcile(node, sources);
    scope.reconcile(node, sources);
    expect(mounted).toHaveLength(1);
    expect(mounted[0]?.widget).toBe(boundary);
    expect(mounted[0]?.node).toBe(owner);
    expect(node.inputs?.[0]?.link).toBe(1);
    scope.reconcile({ title: 'Notes', widgets: [{ name: 'notes', type: 'customtext' }] });
    expect(releases).toBe(1);
  } finally {
    scope.dispose();
    unregister();
  }
});
