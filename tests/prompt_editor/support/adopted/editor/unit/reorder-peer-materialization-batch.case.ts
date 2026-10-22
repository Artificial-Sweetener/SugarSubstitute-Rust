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
/** Actual Scene instantiation shares inert prototypes while literal content, pixels, output and retirement remain per peer. */
import { admitReorderTargetLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderMotionPeerCarriers } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-peer-carriers.ts';
import { PaintNode } from './reorder-paint-node-fixture.ts';
import { usingAtomicPaintFixture, atomicScenePaintSource } from './reorder-atomic-paint-fixture.ts';

test('one Scene batch clones the same actual prototype across two distinct complete LoRA outputs and independent canvases', () =>
  usingAtomicPaintFixture((f) => {
    f.roots[0]!.querySelector('.sp-lora-title')!.textContent = 'First literal';
    f.roots[1]!.querySelector('.sp-lora-title')!.textContent = 'Second literal';
    f.roots[0]!.querySelector('.sp-lora-weight')!.textContent = '0.25';
    f.roots[1]!.querySelector('.sp-lora-weight')!.textContent = '1.75';
    f.roots[0]!.querySelector('.sp-lora-chip')!.dataset.resolution = 'missing';
    const chips = [atomicScenePaintSource(f, 0), atomicScenePaintSource(f, 1)];
    const owner = ReorderMotionPeerCarriers.prepare(
      f.view,
      f.f.line as unknown as HTMLElement,
      chips,
      1,
      1,
      () => true,
    );
    assert.ok(owner);
    const clones: {
      readonly prototype: PaintNode;
      readonly output: PaintNode;
    }[] = [];
    const create = f.f.document.createElement.bind(f.f.document);
    f.f.document.createElement = (tag) => {
      const node = create(tag),
        clone = node.cloneNode.bind(node);
      node.cloneNode = (deep) => {
        const output = clone(deep);
        clones.push({ prototype: node, output });
        return output;
      };
      return node;
    };
    try {
      assert.equal(
        owner.instantiate(() => true),
        true,
      );
      const carriers = f.f.line.querySelectorAll('.sp-reorder-motion-peer');
      assert.equal(carriers.length, 2);
      const first = carriers[0]!.firstElementChild!,
        second = carriers[1]!.firstElementChild!;
      assert.equal(first.dataset.reorderChip, '0');
      assert.equal(second.dataset.reorderChip, '1');
      assert.equal(first.querySelector('.sp-lora-chip')!.dataset.resolution, 'missing');
      assert.equal(second.querySelector('.sp-lora-chip')!.dataset.resolution, 'found');
      assert.equal(first.querySelector('.sp-lora-title')!.textContent, 'First literal');
      assert.equal(second.querySelector('.sp-lora-title')!.textContent, 'Second literal');
      assert.equal(first.querySelector('.sp-lora-weight')!.textContent, '0.25');
      assert.equal(second.querySelector('.sp-lora-weight')!.textContent, '1.75');
      const a = clones.find((pair) => pair.output === first.querySelector('.sp-lora-title')),
        b = clones.find((pair) => pair.output === second.querySelector('.sp-lora-title'));
      assert.ok(a);
      assert.ok(b);
      assert.equal(a.prototype, b.prototype);
      assert.notEqual(a.output, b.output);
      assert.equal(a.prototype.removed, true);
      assert.equal(a.prototype.textContent, '');
      assert.equal(a.prototype.className, '');
      assert.equal(a.output.isConnected, true);
      assert.equal(b.output.isConnected, true);
      a.output.textContent = 'Changed copy only';
      assert.equal(b.output.textContent, 'Second literal');
      assert.equal(f.roots[0]!.querySelector('.sp-lora-title')!.textContent, 'First literal');
      assert.equal(f.draws.length, 2);
      assert.equal(f.draws[0]!.source, f.canvases[0]);
      assert.equal(f.draws[1]!.source, f.canvases[1]);
      assert.notEqual(f.draws[0]!.destination, f.draws[1]!.destination);
      assert.equal(first.querySelectorAll('canvas').length, 1);
      assert.equal(second.querySelectorAll('canvas').length, 1);
      assert.equal(
        carriers.every(
          (carrier) =>
            carrier.style.getPropertyValue('opacity') === '0' &&
            carrier.style.getPropertyPriority('opacity') === 'important',
        ),
        true,
      );
      assert.equal(f.roots[0]!.getAttribute('data-reorder-motion-flow'), null);
      assert.equal(f.roots[1]!.getAttribute('data-reorder-motion-flow'), null);
    } catch (cause) {
      f.causes.push(cause);
    }
    try {
      owner.destroy();
    } catch (cause) {
      f.causes.push(cause);
    }
    assert.equal(f.f.line.querySelectorAll('.sp-reorder-motion-peer').length, 0);
  }));

for (const failure of ['retire', 'body-retire', 'retire-output'] as const)
  test(`Scene batch prototype retirement failure rolls back every peer with failure=${failure}`, () =>
    usingAtomicPaintFixture((f) => {
      const chips = [atomicScenePaintSource(f, 0), atomicScenePaintSource(f, 1)];
      const owner = ReorderMotionPeerCarriers.prepare(
        f.view,
        f.f.line as unknown as HTMLElement,
        chips,
        1,
        1,
        () => true,
      );
      assert.ok(owner);
      const failBody = failure === 'body-retire';
      const primary = new Error('controlled-second-peer-pixels'),
        cleanup = new Error('controlled-shared-prototype-retirement'),
        outputCleanup = new Error('controlled-output-removal');
      const prototypes: PaintNode[] = [];
      const observations: (readonly [string, boolean])[] = [];
      const create = f.f.document.createElement.bind(f.f.document);
      let draws = 0,
        outputFailed = false;
      f.f.document.createElement = (tag) => {
        const node = create(tag),
          clone = node.cloneNode.bind(node);
        node.cloneNode = (deep) => {
          if (!prototypes.includes(node)) {
            prototypes.push(node);
            if (prototypes.length === 1) {
              const remove = node.remove.bind(node);
              node.remove = () => {
                remove();
                observations.push([
                  owner.read(chips, () => true, admitReorderTargetLayout),
                  owner.reveal(() => true),
                ]);
                throw cleanup;
              };
            }
          }
          return clone(deep);
        };
        const removeOutput = node.remove.bind(node);
        node.remove = () => {
          removeOutput();
          if (
            failure === 'retire-output' &&
            !outputFailed &&
            node.className === 'sp-reorder-motion-peer'
          ) {
            outputFailed = true;
            throw outputCleanup;
          }
        };
        if (tag === 'canvas')
          Object.assign(node, {
            getContext: () => ({
              drawImage() {
                if (++draws === 2 && failBody) throw primary;
              },
            }),
          });
        return node;
      };
      try {
        assert.throws(
          () => owner.instantiate(() => true),
          (cause) =>
            failBody
              ? cause instanceof AggregateError &&
                cause.errors[0] === primary &&
                cause.errors[1] === cleanup
              : failure === 'retire-output'
                ? cause instanceof AggregateError &&
                  cause.errors[0] === cleanup &&
                  cause.errors[1] === outputCleanup
                : cause === cleanup,
        );
        assert.deepEqual(observations, [['create', false]]);
        assert.equal(draws, 2);
        assert.ok(prototypes.length > 0);
        assert.equal(
          prototypes.every((node) => node.removed),
          true,
        );
        assert.equal(f.f.line.querySelectorAll('.sp-reorder-motion-peer').length, 0);
        for (const root of f.roots.slice(0, 2)) {
          assert.equal(root.getAttribute('data-reorder-motion-flow'), null);
          assert.equal(root.style.getPropertyValue('opacity'), '');
        }
      } catch (cause) {
        f.causes.push(cause);
      }
      try {
        owner.destroy();
      } catch (cause) {
        f.causes.push(cause);
      }
    }));

test('successful shared prototype retirement cannot republish an owner retired by its native removal', () =>
  usingAtomicPaintFixture((f) => {
    f.roots[0]!.querySelector('.sp-lora-chip')!.dataset.resolution = 'missing';
    const chips = [atomicScenePaintSource(f, 0), atomicScenePaintSource(f, 1)];
    const owner = ReorderMotionPeerCarriers.prepare(
      f.view,
      f.f.line as unknown as HTMLElement,
      chips,
      1,
      1,
      () => true,
    );
    assert.ok(owner);
    const create = f.f.document.createElement.bind(f.f.document);
    let intercepted = false;
    f.f.document.createElement = (tag) => {
      const node = create(tag),
        clone = node.cloneNode.bind(node);
      node.cloneNode = (deep) => {
        if (!intercepted) {
          intercepted = true;
          const remove = node.remove.bind(node);
          node.remove = () => {
            remove();
            owner.destroy();
          };
        }
        return clone(deep);
      };
      return node;
    };
    try {
      assert.equal(
        owner.instantiate(() => true),
        false,
      );
      assert.equal(intercepted, true);
      assert.equal(f.f.line.querySelectorAll('.sp-reorder-motion-peer').length, 0);
      assert.equal(
        owner.read(chips, () => true, admitReorderTargetLayout),
        'unavailable',
      );
    } catch (cause) {
      f.causes.push(cause);
    }
    try {
      owner.destroy();
    } catch (cause) {
      f.causes.push(cause);
    }
  }));
