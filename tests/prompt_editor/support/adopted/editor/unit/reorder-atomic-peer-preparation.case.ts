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
/** Same-request role admission rejects generic native text before complete style/pixel preparation while actual accepted paint remains complete. */
import { admitReorderTargetLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import assert from 'node:assert/strict';
import test from 'node:test';
import { admitNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import {
  prepareNativeReorderPaint,
  preparedAtomicPaintRole,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-preparation.ts';
import { atomicReorderPaintIsland } from '../../../../../../frontend/sugarprompt/editor/src/reorder-atomic-paint-island.ts';
import { copyNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { withReorderPaintMaterialization } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-materialization.ts';
import { ReorderMotionPeerCarriers } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-peer-carriers.ts';
import { usingAtomicPaintFixture, atomicScenePaintSource } from './reorder-atomic-paint-fixture.ts';

test('borrowed outside text refuses the shared predicate without full serialization while accepted LoRA reuses its actual resolved declaration and complete pixels', () =>
  usingAtomicPaintFixture((f) => {
    const rejected = f.roots[0]!,
      accepted = f.roots[1]!,
      lease = f.f.document.createElement('span'),
      island = rejected.firstElementChild!;
    rejected.replaceChildren(lease);
    lease.append(f.f.text('outside shaping text'), island);
    const roots = [rejected, accepted] as unknown as readonly HTMLElement[],
      nodes = admitNativeReorderPaint(roots);
    assert.ok(nodes);
    const legacy = prepareNativeReorderPaint(
      f.view,
      [accepted as unknown as Node],
      nodes,
      'atomic-grab',
    );
    assert.ok(legacy);
    const read = f.f.document.defaultView.getComputedStyle;
    const styleFailure = new Error('rejected-root-full-style-read'),
      mediaFailure = new Error('rejected-root-full-media-read');
    const retained = read(accepted);
    let acceptedReads = 0;
    f.f.document.defaultView.getComputedStyle = (node, pseudo) => {
      if (pseudo) return read(node, pseudo);
      if (node === accepted) {
        if (++acceptedReads > 1) throw new Error('accepted-declaration-not-reused');
        return retained;
      }
      const style = read(node);
      if (!rejected.contains(node)) return style;
      return new Proxy(style, {
        get(target, property, receiver) {
          if (property === 'width' || property === 'font') throw styleFailure;
          if (property === 'getPropertyValue')
            return (name: string) => {
              if (name === 'width' || name === 'font') throw styleFailure;
              return target.getPropertyValue(name);
            };
          return Reflect.get(target, property, receiver);
        },
      });
    };
    Object.defineProperty(f.canvases[0]!, 'width', {
      configurable: true,
      get() {
        throw mediaFailure;
      },
    });
    assert.throws(
      () => prepareNativeReorderPaint(f.view, [rejected as unknown as Node], nodes, 'atomic-grab'),
      (cause) => cause === styleFailure,
    );
    const prepared = prepareNativeReorderPaint(f.view, roots, nodes, 'atomic-peers');
    assert.ok(prepared);
    assert.deepEqual([...prepared], [...legacy]);
    assert.equal(prepared.has(rejected as unknown as Node), false);
    assert.equal(prepared.has(island as unknown as Node), false);
    assert.equal(prepared.has(f.canvases[0]!), false);
    const root = prepared.get(accepted as unknown as Node);
    assert.ok(root && root.kind === 'element');
    assert.equal(root.display, 'inline');
    assert.equal(root.position, 'relative');
    assert.equal(root.inset, '0px');
    assert.equal(root.transform, 'none');
    assert.equal(root.cssText.includes('background-color:rgba(0, 0, 0, 0.055);'), true);
    assert.equal(acceptedReads, 1);
    assert.equal(
      atomicReorderPaintIsland(accepted as unknown as Node, (node) =>
        preparedAtomicPaintRole(prepared.get(node)),
      ),
      accepted.firstElementChild,
    );
    const copy = withReorderPaintMaterialization(
      f.view.dom.ownerDocument,
      (materialization) =>
        copyNativeReorderPaint(
          f.view,
          accepted as unknown as Node,
          prepared,
          materialization,
        ) as HTMLElement | null,
      (output) => output?.remove(),
    );
    assert.ok(copy);
    try {
      assert.equal(copy.textContent, accepted.textContent);
      assert.equal((copy as HTMLElement).querySelectorAll('.sp-lora-title').length, 1);
      assert.equal((copy as HTMLElement).querySelectorAll('.sp-lora-weight').length, 1);
      assert.equal((copy as HTMLElement).querySelectorAll('canvas').length, 1);
      assert.equal(f.draws.length, 1);
      assert.equal(f.draws[0]!.source, f.canvases[1]);
      assert.notEqual(f.draws[0]!.destination, f.canvases[1]);
    } catch (cause) {
      f.causes.push(cause);
    }
    try {
      copy.remove();
    } catch (cause) {
      f.causes.push(cause);
    }
  }));

for (const change of ['class', 'style', 'detach', 'state'] as const)
  test(`eligibility ${change} mutation cannot be recaptured as a fresh full-paint source`, () =>
    usingAtomicPaintFixture((f) => {
      const root = f.roots[0]!,
        nodes = admitNativeReorderPaint([root as unknown as HTMLElement]);
      assert.ok(nodes);
      const read = f.f.document.defaultView.getComputedStyle;
      let fullReads = 0;
      f.f.document.defaultView.getComputedStyle = (node, pseudo) => {
        const style = read(node, pseudo);
        if (pseudo || node !== root) return style;
        return new Proxy(style, {
          get(target, property, receiver) {
            if (property === 'width') {
              fullReads++;
              throw new Error('stale-eligibility-entered-full-paint');
            }
            if (property === 'position') {
              if (change === 'class') root.className += ' changed';
              if (change === 'style') root.style.setProperty('opacity', '0.5');
              if (change === 'detach') root.remove();
              if (change === 'state')
                Object.defineProperty(f.view, 'state', {
                  value: f.view.state.update({}).state,
                });
            }
            return Reflect.get(target, property, receiver);
          },
        });
      };
      assert.equal(
        prepareNativeReorderPaint(f.view, [root as unknown as Node], nodes, 'atomic-peers'),
        null,
      );
      assert.equal(fullReads, 0);
      assert.equal(f.draws.length, 0);
    }));

for (const failure of [null, undefined, new Error('required-atomic-role-read')])
  test(`required eligibility getter preserves raw ${String(failure)}`, () =>
    usingAtomicPaintFixture((f) => {
      const root = f.roots[0]!,
        nodes = admitNativeReorderPaint([root as unknown as HTMLElement]);
      assert.ok(nodes);
      const read = f.f.document.defaultView.getComputedStyle;
      f.f.document.defaultView.getComputedStyle = (node, pseudo) => {
        const style = read(node, pseudo);
        if (pseudo || node !== root) return style;
        return new Proxy(style, {
          get(target, property, receiver) {
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
            if (property === 'display') throw failure;
            return Reflect.get(target, property, receiver);
          },
        });
      };
      let thrown = false,
        caught: unknown;
      try {
        prepareNativeReorderPaint(f.view, [root as unknown as Node], nodes, 'atomic-peers');
      } catch (cause) {
        thrown = true;
        caught = cause;
      }
      assert.equal(thrown, true);
      assert.equal(caught, failure);
      assert.equal(f.draws.length, 0);
    }));

test('whole aggregate pixel admission still refuses a borrowed generic root before any role or full style read', () =>
  usingAtomicPaintFixture((f) => {
    const root = f.roots[0]!,
      island = root.firstElementChild!,
      lease = f.f.document.createElement('span');
    root.replaceChildren(lease);
    lease.append(f.f.text('outside'), island);
    const chips = [atomicScenePaintSource(f, 0), atomicScenePaintSource(f, 1)];
    Object.defineProperty(f.canvases[0]!, 'width', {
      configurable: true,
      value: 1048577,
    });
    f.f.document.defaultView.getComputedStyle = () => {
      throw new Error('inadmissible-aggregate-entered-style-read');
    };
    const allocations = f.f.allocations;
    const owner = ReorderMotionPeerCarriers.prepare(
      f.view,
      f.f.line as unknown as HTMLElement,
      chips,
      1,
      1,
      () => true,
    );
    assert.ok(owner);
    try {
      assert.equal(
        owner.read(chips, () => true, admitReorderTargetLayout),
        'ready',
      );
      assert.equal(owner.handles(0), false);
      assert.equal(owner.handles(1), false);
      assert.equal(
        owner.instantiate(() => true),
        false,
      );
      assert.equal(f.f.allocations, allocations);
      assert.equal(f.draws.length, 0);
    } catch (cause) {
      f.causes.push(cause);
    }
    try {
      owner.destroy();
    } catch (cause) {
      f.causes.push(cause);
    }
  }));

for (const specimen of [
  'one-island',
  'two-islands',
  'outside-text',
  'generated',
  'widget-buffer',
] as const)
  test(`actual ${specimen} eligibility agrees with its independent literal contract before and after minimal preparation`, () =>
    usingAtomicPaintFixture((f) => {
      const root = f.roots[0]!;
      if (specimen === 'two-islands') root.insertBefore(f.roots[1]!.firstElementChild!, null);
      if (specimen === 'outside-text') {
        const island = root.firstElementChild!,
          borrowed = f.f.document.createElement('span');
        root.replaceChildren(borrowed);
        borrowed.append(f.f.text('outside'), island);
      }
      if (specimen === 'widget-buffer') {
        const buffer = f.f.document.createElement('img');
        buffer.className = 'cm-widgetBuffer';
        Object.assign(buffer, {
          complete: true,
          naturalWidth: 0,
          naturalHeight: 0,
          width: 0,
        });
        root.append(buffer);
      }
      const read = f.f.document.defaultView.getComputedStyle;
      f.f.document.defaultView.getComputedStyle = (node, pseudo) => {
        const style = read(node, pseudo);
        if (specimen === 'generated' && node === root && pseudo === '::before')
          return { ...style, content: '"paint"' };
        if (node.className !== 'cm-widgetBuffer') return style;
        return {
          ...style,
          display: 'inline-block',
          getPropertyValue: (name: string) =>
            name === 'display' ? 'inline-block' : style.getPropertyValue(name),
        };
      };
      const nativeRoot = root as unknown as HTMLElement,
        nodes = admitNativeReorderPaint([nativeRoot]);
      assert.ok(nodes);
      const full = prepareNativeReorderPaint(f.view, [nativeRoot], nodes, 'atomic-grab'),
        selected = prepareNativeReorderPaint(f.view, [nativeRoot], nodes, 'atomic-peers');
      assert.ok(full);
      assert.ok(selected);
      const expected = specimen === 'one-island';
      assert.equal(
        atomicReorderPaintIsland(nativeRoot, (node) => preparedAtomicPaintRole(full.get(node))) !==
          null,
        expected,
      );
      assert.equal(selected.has(nativeRoot), expected);
      if (expected) assert.deepEqual([...selected], [...full]);
      assert.equal(f.draws.length, 0);
    }));

test('a later root cannot invalidate an earlier selected source across the mandatory full-paint barrier', () =>
  usingAtomicPaintFixture((f) => {
    const roots = f.roots.slice(0, 2),
      nodes = admitNativeReorderPaint(roots as unknown as readonly HTMLElement[]);
    assert.ok(nodes);
    const read = f.f.document.defaultView.getComputedStyle;
    let changed = false,
      fullReads = 0;
    f.f.document.defaultView.getComputedStyle = (node, pseudo) => {
      const style = read(node, pseudo);
      if (pseudo || !roots.includes(node)) return style;
      return new Proxy(style, {
        get(target, property, receiver) {
          if (property === 'width') {
            fullReads++;
            throw new Error('cross-root-stale-full-paint');
          }
          if (node === roots[1] && property === 'position') {
            changed = true;
            roots[0]!.className += ' changed-by-later-root';
          }
          return Reflect.get(target, property, receiver);
        },
      });
    };
    const allocations = f.f.allocations;
    assert.equal(
      prepareNativeReorderPaint(f.view, roots as unknown as readonly Node[], nodes, 'atomic-peers'),
      null,
    );
    assert.equal(changed, true);
    assert.equal(fullReads, 0);
    assert.equal(f.f.allocations, allocations);
    assert.equal(f.draws.length, 0);
  }));

test('selection reuses the same complete actual output without repeatedly rescanning an earlier root between later eligibility reads', () =>
  usingAtomicPaintFixture((f) => {
    const roots = f.roots.slice(0, 4),
      nativeRoots = roots as unknown as readonly HTMLElement[],
      nodes = admitNativeReorderPaint(nativeRoots);
    assert.ok(nodes);
    const expected = prepareNativeReorderPaint(f.view, nativeRoots, nodes, 'atomic-grab');
    assert.ok(expected);
    const read = f.f.document.defaultView.getComputedStyle,
      attribute = roots[0]!.getAttribute.bind(roots[0]!);
    let laterStarted = false,
      selected = false;
    roots[0]!.getAttribute = (name) => {
      if (name === 'class' && laterStarted && !selected)
        throw new Error('redundant-prefix-stamp-scan');
      return attribute(name);
    };
    f.f.document.defaultView.getComputedStyle = (node, pseudo) => {
      const style = read(node, pseudo);
      if (!pseudo && node === roots[1]) laterStarted = true;
      if (pseudo || node !== roots[3]!.firstElementChild) return style;
      return new Proxy(style, {
        get(target, property, receiver) {
          if (property === 'transform') selected = true;
          return Reflect.get(target, property, receiver);
        },
      });
    };
    try {
      const actual = prepareNativeReorderPaint(f.view, nativeRoots, nodes, 'atomic-peers');
      assert.ok(actual);
      assert.equal(laterStarted, true);
      assert.equal(selected, true);
      assert.deepEqual([...actual], [...expected]);
      assert.equal(f.draws.length, 0);
    } catch (cause) {
      f.causes.push(cause);
    }
    roots[0]!.getAttribute = attribute;
  }));
