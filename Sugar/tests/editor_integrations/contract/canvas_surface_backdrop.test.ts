/** Prove bounded acrylic negotiation preserves older public surface contracts. */
import type { CanvasContext } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasSurfaceCapabilities } from '../../../integrations/contracts/CanvasSurface.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import {
  negotiatesCanvasSurfaceBackdrop,
  supportsCanvasSurfaceBackdrop,
} from '../../../integrations/shared/CanvasSurfaceBackdrop.js';
import {
  dispatchCanvasSurfaceDiscovery,
  EMPTY_CANVAS_SURFACE_CAPABILITIES,
  snapshotCanvasSurfaceCapabilities,
} from '../../../integrations/shared/CanvasSurfaceDiscovery.js';
import { isCanvasSurfaceGeometry } from '../../../integrations/shared/CanvasSurfaceValidation.generated.js';

const capabilities: CanvasSurfaceCapabilities = {
  ...EMPTY_CANVAS_SURFACE_CAPABILITIES,
  schema: 3,
  backdrop: { schema: 1 },
};
const geometry = {
  coordinateSpace: 'client',
  units: 'css-pixels',
  origin: { x: 0, y: 0 },
  viewport: { width: 800, height: 600 },
  clip: { x: 0, y: 0, width: 800, height: 600 },
  physicalPixelsPerCssPixel: 1.5,
  visible: true,
  focused: false,
  backdrops: [{ x: 8, y: 552, width: 240, height: 40, cornerRadius: 8, sigma: 10 }],
} as const;

test('requires explicit current native material support and bounds every region', () => {
  expect(negotiatesCanvasSurfaceBackdrop(capabilities)).toBe(true);
  expect(negotiatesCanvasSurfaceBackdrop({ ...capabilities, schema: 2 })).toBe(false);
  expect(negotiatesCanvasSurfaceBackdrop(EMPTY_CANVAS_SURFACE_CAPABILITIES)).toBe(false);
  const command = {
    type: 'surface.geometry',
    revision: 2,
    address: { sessionId: 's', viewId: 'v', windowId: 'w', attachmentId: 'a', generation: 1 },
    geometry,
  } as const;
  expect(supportsCanvasSurfaceBackdrop(capabilities, command)).toBe(true);
  expect(supportsCanvasSurfaceBackdrop(EMPTY_CANVAS_SURFACE_CAPABILITIES, command)).toBe(false);
  expect(isCanvasSurfaceGeometry(geometry)).toBe(true);
  for (const backdrops of [
    [geometry.backdrops[0], geometry.backdrops[0], geometry.backdrops[0]],
    [{ ...geometry.backdrops[0], sigma: Infinity }],
    [{ ...geometry.backdrops[0], sigma: -1 }],
    [{ ...geometry.backdrops[0], nativeHandle: 1 }],
  ])
    expect(isCanvasSurfaceGeometry({ ...geometry, backdrops })).toBe(false);
});

test('snapshots the nested optional material declaration', () => {
  const source: CanvasSurfaceCapabilities = { ...capabilities, backdrop: { schema: 1 } };
  const retained = snapshotCanvasSurfaceCapabilities(source);
  expect(retained.backdrop).toEqual({ schema: 1 });
  expect(retained.backdrop).not.toBe(source.backdrop);
  expect(Object.isFrozen(retained.backdrop)).toBe(true);
});

test.each([2, 3] as const)(
  'negotiates schema %i without sending later fields to older clients',
  async (schema) => {
    const context: CanvasContext = {
      canvas: {
        id: 'native',
        label: 'Native',
        capabilities: {
          displayOutput: true,
          insertImageLayer: false,
          positionImageLayer: false,
        },
      },
      document: null,
    };
    const reply = await dispatchCanvasSurfaceDiscovery(
      {
        protocol: 'sugarcubes.canvas',
        version: 3,
        session: 's',
        requestId: 1,
        type: 'surface.capabilities.request',
        schema,
      },
      new CanvasCancellation(),
      {
        surface: { capabilities, execute: async () => ({ status: 'applied', message: '' }) },
        context: async () => context,
        report: (error) => {
          throw error;
        },
      },
    );
    expect(reply.status).toBe('available');
    expect(reply.capabilities.schema).toBe(schema);
    expect(reply.capabilities.backdrop).toEqual(schema === 3 ? { schema: 1 } : undefined);
  },
);
