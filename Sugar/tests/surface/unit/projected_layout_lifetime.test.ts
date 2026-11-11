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
/** Prove projected native-card teardown preserves the live graph's reactive layout reference. */
import { expect, test } from '@jest/globals';
import { createContext, SourceTextModule } from 'node:vm';
import {
  buildNativeNodeComponentModule,
  NativeRendererCompatibilityError,
} from '../../../frontend/comfyui/ui/surface/ComfyRuntimeModuleLoader.js';

const graphViewUrl = 'http://127.0.0.1:8237/assets/GraphView-fixture.js';
const nativeLayoutSource = [
  'function useNodeLayout(e){let t=e,r=store.getNodeLayoutRef(t);',
  'onUnmounted(()=>{store.cleanupNodeRef(t)});return r}',
  'const Native=defineComponent({__name:`LGraphNode`,setup:useNodeLayout});',
  'export{Native};',
].join('');

/** Evaluate the installed-module boundary against Comfy's shared reference-cache contract. */
async function mountFixture(source: string): Promise<{
  reference: { x: number };
  move(x: number): void;
  remount(): void;
  unmount(): void;
}> {
  const refs = new Map<string, { x: number }>();
  const callbacks: Array<() => void> = [];
  const store = {
    getNodeLayoutRef(id: string): { x: number } {
      let ref = refs.get(id);
      if (!ref) {
        ref = { x: 0 };
        refs.set(id, ref);
      }
      return ref;
    },
    cleanupNodeRef(id: string): void {
      refs.delete(id);
    },
  };
  const reference = store.getNodeLayoutRef('loose');
  const module = new SourceTextModule(source, {
    context: createContext({
      store,
      onUnmounted: (callback: () => void) => callbacks.push(callback),
      defineComponent: (component: unknown) => component,
    }),
  });
  await module.link(() => {
    throw new Error('Fixture unexpectedly requested a runtime dependency.');
  });
  await module.evaluate();
  const component: unknown = Reflect.get(module.namespace, 'Native');
  if (
    !component ||
    typeof component !== 'object' ||
    !('setup' in component) ||
    typeof component.setup !== 'function'
  )
    throw new Error('Fixture did not expose its native component setup boundary.');
  const setup = component.setup;
  expect(component.setup('loose')).toBe(reference);
  return {
    reference,
    move: (x) => {
      store.getNodeLayoutRef('loose').x = x;
    },
    remount: () => {
      expect(setup('loose')).toBe(reference);
    },
    unmount: () => callbacks.splice(0).forEach((callback) => callback()),
  };
}

test('characterizes native graph teardown invalidating its layout reference', async () => {
  const fixture = await mountFixture(nativeLayoutSource);
  fixture.unmount();
  fixture.move(50);
  expect(fixture.reference.x).toBe(0);
});

test('closing projected masonry retains the graph reference through repeated mounts and movement', async () => {
  const fixture = await mountFixture(
    buildNativeNodeComponentModule(nativeLayoutSource, graphViewUrl),
  );
  fixture.unmount();
  fixture.move(50);
  expect(fixture.reference.x).toBe(50);
  fixture.remount();
  fixture.unmount();
  fixture.move(100);
  expect(fixture.reference.x).toBe(100);
});

test('retains unrelated component teardown and supports runtimes without layout cleanup', () => {
  const unrelated = 'function other(){onUnmounted(()=>{store.cleanupNodeRef("other")})}';
  const transformed = buildNativeNodeComponentModule(nativeLayoutSource + unrelated, graphViewUrl);
  expect(transformed).toContain(unrelated);
  const older = nativeLayoutSource.replace('onUnmounted(()=>{store.cleanupNodeRef(t)});', '');
  expect(buildNativeNodeComponentModule(older, graphViewUrl)).toContain('export{Native as');
});

test('rejects changed native layout cleanup instead of silently breaking graph gestures', () => {
  const changed = nativeLayoutSource.replace(
    'store.cleanupNodeRef(t)',
    'store.cleanupNodeRef(t, true)',
  );
  expect(() => buildNativeNodeComponentModule(changed, graphViewUrl)).toThrow(
    NativeRendererCompatibilityError,
  );
});
