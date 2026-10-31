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
/** Prove a refused native surface at startup falls back to the browser presenter silently. */

import { jest } from '@jest/globals';
import { EmbeddedCanvasConnection } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedCanvasConnection.js';
import {
  createNativePanelTransport,
  prefersNativeOutputSurface,
} from '../../../frontend/comfyui/ui/core/embedded/NativePanelTransport.js';
import { OutputPresenterSelection } from '../../../frontend/comfyui/ui/output_canvas/application/OutputPresenterSelection.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import type { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import type { CanvasAttachmentAddress } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import {
  CanvasPrimarySurface,
  type PrimaryCanvasAttachment,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasPrimarySurface.js';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { nativeCanvasRendererKey } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasCapabilities.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { EMPTY_CANVAS_SURFACE_CAPABILITIES } from '../../../integrations/shared/CanvasSurfaceDiscovery.js';
import { selectedPngCapabilities } from '../../editor_integrations/support/canvas_surface.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';

type Discovery = 'available' | 'unsupported' | 'unavailable' | 'no-display';

/** A browser presenter double that records only its construction. */
function browserAttachment(): PrimaryCanvasAttachment {
  return {
    diagnostics: () => ({
      displayed: 0,
      pendingDecodes: 0,
      queuedInputs: 0,
      previewDrops: 0,
      estimatedBytes: 0,
      unknownLoads: 0,
      activeDeadlines: 0,
      renderFrames: 0,
    }),
    refresh: () => {},
    wants: () => false,
    receive: () => {},
    dispose: () => {},
  };
}

/** Compose the real connection, presenter selection and primary surface over a fake native bridge. */
function fixture(discovery: Discovery) {
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    width: 800,
    height: 600,
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 800,
    bottom: 600,
    toJSON: () => ({}),
  });
  for (const key of ['offsetWidth', 'clientWidth'] as const)
    jest.spyOn(HTMLElement.prototype, key, 'get').mockReturnValue(800);
  for (const key of ['offsetHeight', 'clientHeight'] as const)
    jest.spyOn(HTMLElement.prototype, key, 'get').mockReturnValue(600);
  const catalog: CanvasCatalogSnapshot = canvasCatalogFixture();
  let keys = 0;
  const views = new CanvasViewRegistry(() => String(++keys));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  selection.reconcile(view, catalog);
  selection.apply(view, catalog, {
    type: 'select-source',
    selection: { sourceId: catalog.entries[0]!.source.id, batchIndex: 0 },
  });
  const root = document.createElement('section');
  document.body.append(root);
  const clock = new WorkflowTestClock();
  const frames = new CanvasFrameFixture();
  const errors: unknown[] = [];
  const listeners = new Set<(value: unknown) => void>();
  const messages: CanvasMessage[] = [];
  const reply = (value: unknown) => {
    for (const receive of listeners) receive(value);
  };
  const bridge = {
    host: 'ai.artificialsweetener.sugarsubstitute',
    session: 'presenter-fallback-session',
    preferencePolicy: 'saved',
    outputSurfacePreference: 'host',
    send: (message: CanvasMessage) => {
      messages.push(message);
      if (message.type === 'context.request')
        reply({
          ...message,
          type: 'context.reply',
          context: {
            canvas: {
              id: 'native',
              label: 'Native',
              capabilities: {
                displayOutput: discovery !== 'no-display',
                insertImageLayer: false,
                positionImageLayer: false,
              },
            },
            document: null,
          },
        });
      if (message.type === 'surface.capabilities.request')
        reply({
          protocol: message.protocol,
          version: message.version,
          session: message.session,
          requestId: message.requestId,
          type: 'surface.capabilities.reply',
          status: discovery,
          capabilities:
            discovery === 'available' ? selectedPngCapabilities : EMPTY_CANVAS_SURFACE_CAPABILITIES,
          message: '',
        });
      if (message.type === 'surface.prepare' || message.type === 'surface.release')
        reply({
          protocol: message.protocol,
          version: message.version,
          session: message.session,
          requestId: message.requestId,
          type: 'surface.reply',
          address: message.address,
          revision: message.revision,
          status: message.type === 'surface.prepare' ? 'ready' : 'applied',
          message: '',
        });
    },
    subscribe: (receive: (value: unknown) => void) => {
      listeners.add(receive);
      return () => {
        listeners.delete(receive);
      };
    },
  };
  Object.defineProperty(window, 'sugarcubesPanelHost', { value: bridge, configurable: true });
  const transport = createNativePanelTransport(window);
  if (!transport) throw new Error('Native transport fixture was not admitted.');
  const connection = new EmbeddedCanvasConnection();
  const presenter = new OutputPresenterSelection({
    prefersNative: () => prefersNativeOutputSurface(window),
    surface: () => connection.surface,
    discovered: () => connection.surfaceDiscovered,
  });
  const mounted = { native: 0, browser: 0 };
  const natives: { attachment: NativeCanvasAttachment; address: CanvasAttachmentAddress }[] = [];
  const primary = new CanvasPrimarySurface({
    document,
    views,
    window: windowId('presenter'),
    coordinator: {
      active: () => view,
      catalog: () => catalog,
      subscribe: () => () => {},
    } as unknown as CanvasSessionCoordinator,
    element: () => root,
    renderer: () =>
      presenter.current() === 'browser' ? 'browser' : nativeCanvasRendererKey(connection.surface),
    report: (error) => errors.push(error),
    mount: (element, address, _catalog, accepts, acceptsPreparation) => {
      if (presenter.current() === 'browser') {
        mounted.browser++;
        return Promise.resolve(browserAttachment());
      }
      mounted.native++;
      const attachment = new NativeCanvasAttachment(element, {
        address,
        views,
        selection,
        inspections: new CanvasInspectionStore(),
        clock,
        frames,
        catalog: () => catalog,
        accepts,
        acceptsPreparation,
        connected: () => true,
        open: () => {},
        report: (error) => errors.push(error),
        nativeSurface: () => connection.surface,
        surfacePending: () => presenter.current() === 'pending',
      });
      natives.push({ attachment, address });
      return attachment;
    },
  });
  document.addEventListener('sugarcubes-canvas-discovered', primary.reconcile);
  const settle = async (turns = 48) => {
    for (let turn = 0; turn < turns; turn++) await Promise.resolve();
  };
  const dispose = async () => {
    document.removeEventListener('sugarcubes-canvas-discovered', primary.reconcile);
    primary.dispose();
    await settle(8);
    connection.dispose();
    Reflect.deleteProperty(window, 'sugarcubesPanelHost');
  };
  /** Read every status a native attachment published, as its overlay renders it. */
  const published = () =>
    natives.map(({ attachment }) => JSON.stringify(attachment.port.read())).join('\n') +
    (root.textContent ?? '');
  return {
    published,
    connection,
    transport,
    primary,
    presenter,
    mounted,
    errors,
    messages,
    settle,
    dispose,
  };
}

/** Collect every status or error text the attachment reported to the user. */
function reportedText(errors: readonly unknown[]): string[] {
  return errors.map((error) => (error instanceof Error ? error.message : String(error)));
}

describe('output presenter at native startup', () => {
  afterEach(() => {
    document.body.replaceChildren();
    jest.restoreAllMocks();
  });

  test.each(['unsupported', 'unavailable', 'no-display'] as const)(
    'a %s native discovery selects the browser presenter without unavailable text',
    async (discovery) => {
      const f = fixture(discovery);
      try {
        f.primary.setup();
        await f.settle();
        expect(f.presenter.current()).toBe('pending');
        expect(f.mounted.browser).toBe(0);
        expect(reportedText(f.errors).join('\n')).not.toMatch(/unavailable/i);
        expect(f.published()).not.toMatch(/unavailable/i);
        f.connection.connect(f.transport, window, (error) => f.errors.push(error));
        await f.settle();
        expect(f.connection.surfaceDiscovered).toBe(true);
        expect(f.connection.surface).toBeNull();
        expect(f.presenter.current()).toBe('browser');
        expect(f.mounted.browser).toBe(1);
        expect(reportedText(f.errors).join('\n')).not.toMatch(/unavailable/i);
        // Only the pending native placeholder preceded the browser presenter.
        expect(f.mounted.native).toBe(1);
      } finally {
        await f.dispose();
      }
    },
  );

  test('an available native surface keeps the browser presenter unloaded', async () => {
    const f = fixture('available');
    try {
      f.primary.setup();
      expect(f.presenter.current()).toBe('pending');
      f.connection.connect(f.transport, window, (error) => f.errors.push(error));
      await f.settle();
      expect(f.connection.surface).not.toBeNull();
      expect(f.presenter.current()).toBe('native');
      expect(f.mounted.browser).toBe(0);
      // The pending placeholder is replaced by an attachment bound to the negotiated surface.
      expect(f.mounted.native).toBe(2);
      expect(f.messages.some((message) => message.type === 'surface.capabilities.request')).toBe(
        true,
      );
      expect(reportedText(f.errors).join('\n')).not.toMatch(/unavailable/i);
      // A later context refresh renegotiates without loading the browser presenter.
      f.connection.refresh();
      await f.settle();
      expect(f.presenter.current()).toBe('native');
      expect(f.mounted.browser).toBe(0);
    } finally {
      await f.dispose();
    }
  });
});
