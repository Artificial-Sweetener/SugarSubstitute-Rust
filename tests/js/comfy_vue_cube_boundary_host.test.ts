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
/** Verify Cube faces reposition Comfy's real Nodes 2.0 boundary slots. */

import { jest } from '@jest/globals';

import { ComfyVueCubeBoundaryHost } from '../../frontend/comfyui/ui/surface/ComfyVueCubeBoundaryHost.js';
import { CubePortPresentationController } from '../../frontend/comfyui/ui/cube/connection/CubePortPresentationController.js';
import type { ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';

describe('ComfyVueCubeBoundaryHost', () => {
  test('places outputs across from preview titles while retaining native input rows', () => {
    const body = document.createElement('div');
    setRect(body, { top: 100, left: 20, width: 250, height: 300 });
    Object.defineProperty(body, 'offsetWidth', { configurable: true, value: 500 });
    const row = document.createElement('div');
    setRect(row, { top: 100, left: 20, width: 250, height: 300 });
    Object.defineProperty(row, 'offsetWidth', { configurable: true, value: 500 });
    const firstInput = slot('input');
    const secondInput = slot('input');
    setRect(firstInput, { top: 110, left: 20, width: 80, height: 20 });
    setRect(secondInput, { top: 150, left: 20, width: 80, height: 20 });
    const output = slot('output');
    row.append(firstInput, secondInput, output);
    const previewTitle = document.createElement('header');
    previewTitle.dataset.cubePreviewOutputTitle = '';
    setRect(previewTitle, { top: 150, left: 200, width: 60, height: 20 });
    body.append(row, previewTitle);

    const host = new ComfyVueCubeBoundaryHost({ body, node: node() });

    expect(row.dataset.sugarcubeBoundaryRow).toBe('');
    expect(firstInput.dataset.sugarcubeBoundaryDirection).toBe('input');
    expect(firstInput.dataset.sugarcubeBoundaryIndex).toBe('0');
    expect(firstInput.style.getPropertyValue('--sugarcube-boundary-position')).toBe('40px');
    expect(secondInput.dataset.sugarcubeBoundaryDirection).toBe('input');
    expect(secondInput.dataset.sugarcubeBoundaryIndex).toBe('1');
    expect(secondInput.style.getPropertyValue('--sugarcube-boundary-position')).toBe('120px');
    expect(output.dataset.sugarcubeBoundaryDirection).toBe('output');
    expect(output.dataset.sugarcubeBoundaryIndex).toBe('0');
    expect(output.style.getPropertyValue('--sugarcube-boundary-position')).toBe('120px');

    host.dispose();
  });

  test('registers the complete Cube body as the magnetic travel range', () => {
    const body = document.createElement('div');
    setRect(body, { top: 100, left: 20, width: 500, height: 600 });
    Object.defineProperty(body, 'offsetWidth', { configurable: true, value: 500 });
    const root = document.createElement('article');
    root.className = 'lg-node';
    setRect(root, { top: 60, left: 20, width: 500, height: 680 });
    Object.defineProperty(root, 'offsetWidth', { configurable: true, value: 500 });
    const row = document.createElement('div');
    setRect(row, { top: 100, left: 20, width: 500, height: 600 });
    const input = slot('input');
    setRect(input, { top: 120, left: 20, width: 80, height: 20 });
    row.append(input);
    body.append(row);
    root.append(body);
    const controller = new CubePortPresentationController({ requestFrame: () => null });
    const register = jest.spyOn(controller, 'register');

    const host = new ComfyVueCubeBoundaryHost({
      body,
      node: node([{ type: 'IMAGE' }]),
      portPresentation: controller,
    });

    expect(register).toHaveBeenCalledWith(expect.anything(), 'input', [
      { index: 0, defaultY: 40, minY: 22, maxY: 598, labelY: 40 },
    ]);
    host.dispose();
  });

  test('resolves DOM socket centers in graph space without the Vue title offset', () => {
    const root = document.createElement('article');
    root.className = 'lg-node';
    root.style.transform = 'translate(0px, 70px)';
    setRect(root, { top: 70, left: 20, width: 500, height: 380 });
    Object.defineProperty(root, 'offsetWidth', { configurable: true, value: 500 });
    const body = document.createElement('div');
    setRect(body, { top: 100, left: 20, width: 500, height: 320 });
    Object.defineProperty(body, 'offsetWidth', { configurable: true, value: 500 });
    const row = document.createElement('div');
    setRect(row, { top: 100, left: 20, width: 500, height: 320 });
    const output = slot('output');
    row.append(output);
    const previewTitle = document.createElement('header');
    previewTitle.dataset.cubePreviewOutputTitle = '';
    setRect(previewTitle, { top: 120, left: 200, width: 60, height: 20 });
    body.append(row, previewTitle);
    root.append(body);
    const cubeNode = node();
    cubeNode.pos = [0, 120];
    const controller = new CubePortPresentationController({ requestFrame: () => null });

    const host = new ComfyVueCubeBoundaryHost({
      body,
      node: cubeNode,
      portPresentation: controller,
    });

    expect(output.style.getPropertyValue('--sugarcube-boundary-position')).toBe('30px');
    expect(controller.resolveGraphPosition(cubeNode, 'output', 0, [500, 0])).toEqual([500, 130]);
    host.dispose();
  });

  test('restores host presentation and reconciles slots replaced by Vue', () => {
    const body = document.createElement('div');
    const firstRow = document.createElement('div');
    const firstSlot = slot('input');
    firstSlot.setAttribute('style', 'color: red');
    firstSlot.dataset.sugarcubeBoundaryDirection = 'preserved';
    firstRow.append(firstSlot);
    body.append(firstRow);
    const host = new ComfyVueCubeBoundaryHost({ body, node: node() });
    const secondRow = document.createElement('div');
    const secondSlot = slot('output');
    secondRow.append(secondSlot);

    firstRow.replaceWith(secondRow);
    host.reconcile();

    expect(secondRow.dataset.sugarcubeBoundaryRow).toBe('');
    expect(secondSlot.dataset.sugarcubeBoundaryDirection).toBe('output');
    host.dispose();
    expect(firstRow.dataset.sugarcubeBoundaryRow).toBeUndefined();
    expect(firstSlot.getAttribute('style')).toBe('color: red');
    expect(firstSlot.dataset.sugarcubeBoundaryDirection).toBe('preserved');
    expect(secondRow.dataset.sugarcubeBoundaryRow).toBeUndefined();
    expect(secondSlot.dataset.sugarcubeBoundaryDirection).toBeUndefined();
    expect(secondSlot.getAttribute('style')).toBeNull();
  });

  test('requests native slot remeasurement only when boundary presentation changes', () => {
    const body = document.createElement('div');
    const row = document.createElement('div');
    row.append(slot('input'), slot('output'));
    body.append(row);
    const requestSlotLayoutSync = jest.fn();

    const host = new ComfyVueCubeBoundaryHost({
      body,
      node: node(),
      requestSlotLayoutSync,
    });
    host.reconcile();
    expect(requestSlotLayoutSync).toHaveBeenCalledTimes(1);

    const replacement = document.createElement('div');
    replacement.append(slot('input'));
    row.replaceWith(replacement);
    host.reconcile();
    host.reconcile();

    expect(requestSlotLayoutSync).toHaveBeenCalledTimes(2);
  });

  test('does not remeasure when Vue replaces slots with identical semantic geometry', () => {
    const body = document.createElement('div');
    const row = document.createElement('div');
    row.append(slot('input'), slot('output'));
    body.append(row);
    const requestSlotLayoutSync = jest.fn();
    const host = new ComfyVueCubeBoundaryHost({
      body,
      node: node(),
      requestSlotLayoutSync,
    });
    const replacement = document.createElement('div');
    replacement.append(slot('input'), slot('output'));

    row.replaceWith(replacement);
    host.reconcile();

    expect(requestSlotLayoutSync).toHaveBeenCalledTimes(1);
    host.dispose();
  });

  test('renders input types literally without mutating an already-presented label', async () => {
    const body = document.createElement('div');
    const row = document.createElement('div');
    const input = slot('input');
    const label = document.createElement('span');
    label.className = 'text-node-component-slot-text';
    label.textContent = 'input.image';
    input.append(label);
    row.append(input);
    body.append(row);

    const host = new ComfyVueCubeBoundaryHost({
      body,
      node: node([{ type: '<b>IMAGE</b>' }]),
    });

    expect(label.textContent).toBe('<b>IMAGE</b>');
    expect(label.querySelector('b')).toBeNull();
    const mutations = jest.fn();
    const observer = new MutationObserver(mutations);
    observer.observe(label, { childList: true, subtree: true });

    host.reconcile();
    host.reconcile();
    await Promise.resolve();
    await Promise.resolve();

    expect(mutations).not.toHaveBeenCalled();
    observer.disconnect();
    host.dispose();
    expect(label.textContent).toBe('input.image');
  });

  test('remeasures native slot layout once after magnetic animation instead of every frame', () => {
    let now = 0;
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: () => null,
      invalidate: () => undefined,
      durationMs: 180,
    });
    const outputNode: ComfyNode = {
      id: 'output',
      pos: [0, 0],
      size: [500, 320],
      outputs: [{ type: 'IMAGE' }],
    };
    const inputNode: ComfyNode = {
      id: 'input',
      pos: [540, 80],
      size: [500, 320],
      inputs: [{ type: 'IMAGE' }],
    };
    const outputBody = boundaryBody('output');
    const inputBody = boundaryBody('input');
    setRect(inputBody.querySelector<HTMLElement>('.lg-slot--input')!, {
      top: 130,
      left: 0,
      width: 80,
      height: 20,
    });
    const outputSync = jest.fn();
    const inputSync = jest.fn();
    const outputHost = new ComfyVueCubeBoundaryHost({
      body: outputBody,
      node: outputNode,
      portPresentation: controller,
      requestSlotLayoutSync: outputSync,
    });
    const inputHost = new ComfyVueCubeBoundaryHost({
      body: inputBody,
      node: inputNode,
      portPresentation: controller,
      requestSlotLayoutSync: inputSync,
    });
    const register = jest.spyOn(controller, 'register');
    controller.updateMatches([
      {
        outputId: 'output',
        outputSlot: 0,
        outputNode,
        outputCube: 'output-definition',
        outputPos: [500, 160],
        inputId: 'input',
        inputSlot: 0,
        inputNode,
        inputCube: 'input-definition',
        inputName: 'image',
        inputPos: [540, 240],
        originId: 'producer',
        originSlot: 0,
        promptTargets: [{ nodeId: 'consumer', inputSlot: 0, inputName: 'image' }],
        distance: 40,
      },
    ]);

    expect(outputSync).toHaveBeenCalledTimes(1);
    expect(inputSync).toHaveBeenCalledTimes(1);
    expect(register).not.toHaveBeenCalled();

    now = 180;
    outputHost.reconcile();
    inputHost.reconcile();

    expect(register).toHaveBeenCalledTimes(4);
    expect(outputSync).toHaveBeenCalledTimes(2);
    expect(inputSync).toHaveBeenCalledTimes(2);
    outputHost.dispose();
    inputHost.dispose();
  });

  it('restores the Vue graph origin after another renderer releases shared presentation state', () => {
    const controller = new CubePortPresentationController({
      now: () => 0,
      requestFrame: () => null,
    });
    const cube = node();
    cube.pos = [0, 999];
    const body = boundaryBody('input');
    const root = document.createElement('div');
    root.className = 'lg-node';
    root.style.transform = 'translate(0px, 70px)';
    root.append(body);
    document.body.append(root);
    const host = new ComfyVueCubeBoundaryHost({
      body,
      node: cube,
      titleHeight: 30,
      portPresentation: controller,
    });

    controller.release(cube);
    controller.register(cube, 'input', [{ index: 0, defaultY: 0, minY: 0, maxY: 300, labelY: 0 }]);
    host.reconcile();

    expect(controller.resolveGraphPosition(cube, 'input', 0, [0, 0])?.[1]).toBeCloseTo(100, 3);
    host.dispose();
  });

  it('rebases stable input anchors when Comfy moves the native boundary row', () => {
    const root = document.createElement('article');
    root.className = 'lg-node';
    root.style.transform = 'translate(0px, 70px)';
    setRect(root, { top: 70, left: 20, width: 500, height: 380 });
    Object.defineProperty(root, 'offsetWidth', { configurable: true, value: 500 });
    const body = document.createElement('div');
    setRect(body, { top: 100, left: 20, width: 500, height: 320 });
    Object.defineProperty(body, 'offsetWidth', { configurable: true, value: 500 });
    const row = document.createElement('div');
    setRect(row, { top: 100, left: 20, width: 500, height: 320 });
    const input = slot('input');
    setRect(input, { top: 102, left: 20, width: 80, height: 20 });
    row.append(input);
    body.append(row);
    root.append(body);
    const cubeNode = node([{ type: 'IMAGE' }]);
    const controller = new CubePortPresentationController({ requestFrame: () => null });
    const host = new ComfyVueCubeBoundaryHost({
      body,
      node: cubeNode,
      portPresentation: controller,
    });
    const initialPosition = input.style.getPropertyValue('--sugarcube-boundary-position');
    const initialGraphY = controller.resolveDefaultGraphPosition(cubeNode, 'input', 0, [0, 0])[1];

    setRect(body, { top: 112, left: 20, width: 500, height: 320 });
    setRect(row, { top: 112, left: 20, width: 500, height: 320 });
    host.reconcile();

    expect(input.style.getPropertyValue('--sugarcube-boundary-position')).toBe(initialPosition);
    expect(controller.resolveDefaultGraphPosition(cubeNode, 'input', 0, [0, 0])[1]).toBe(
      initialGraphY + 12,
    );
    host.dispose();
  });
});

/** Build one real native slot-shaped element for a direction. */
function slot(direction: 'input' | 'output'): HTMLDivElement {
  const element = document.createElement('div');
  element.className = `lg-slot lg-slot--${direction}`;
  return element;
}

/** Build the finite node position consumed by boundary presentation. */
function node(inputs: Array<{ type: string }> = []) {
  return { id: 'cube', pos: [0, 0], size: [500, 320], inputs };
}

/** Build one finite native boundary row for animation remeasurement tests. */
function boundaryBody(direction: 'input' | 'output'): HTMLDivElement {
  const body = document.createElement('div');
  setRect(body, { top: 0, left: 0, width: 500, height: 320 });
  Object.defineProperty(body, 'offsetWidth', { configurable: true, value: 500 });
  const row = document.createElement('div');
  setRect(row, { top: 0, left: 0, width: 500, height: 320 });
  row.append(slot(direction));
  body.append(row);
  return body;
}

/** Give one JSDOM element finite viewport geometry. */
function setRect(
  element: HTMLElement,
  rect: { top: number; left: number; width: number; height: number },
): void {
  element.getBoundingClientRect = () =>
    ({
      x: rect.left,
      y: rect.top,
      top: rect.top,
      left: rect.left,
      width: rect.width,
      height: rect.height,
      right: rect.left + rect.width,
      bottom: rect.top + rect.height,
      toJSON: () => ({}),
    }) as DOMRect;
}
