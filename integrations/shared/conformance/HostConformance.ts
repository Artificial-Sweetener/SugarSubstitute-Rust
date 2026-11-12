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
 * Run the SugarCubes host conformance kit against a host's own panel channel.
 *
 * Entry point: {@link runHostConformance}. A host in any repository passes an adapter over the
 * channel it gives the SugarCubes page, and the kit plays the page: it announces the panel, reads
 * the host description and presentation, then exercises each protocol and capability the host
 * declares. Every message the host sends is checked against its generated schema guard, and the
 * first invalid one fails the run at once. The kit depends only on the contract schemas and
 * portable shared code; it uses no ComfyUI, browser, filesystem or timer API.
 */
import { HOST_CAPABILITIES, HOST_PROTOCOLS } from '../../contracts/HostDescription.js';
import type { HostDescriptionMessage } from '../../contracts/HostProtocol.generated.js';
import type { PanelPresentation } from '../../contracts/PanelProtocol.generated.js';
import type { CanvasSurfaceCapabilitiesReply } from '../../contracts/CanvasSurfaceProtocol.generated.js';
import type { UpdatesStateMessage } from '../../contracts/UpdatesProtocol.generated.js';
import type { MemoryAllowanceMessage } from '../../contracts/MemoryProtocol.generated.js';
import type { CanvasCancellationSource } from '../../contracts/CanvasHost.js';
import { DeclaredHost } from '../HostDescription.js';
import { isHostDescriptionMessage } from '../HostValidation.generated.js';
import { isPanelConfigure, isPanelPing } from '../PanelValidation.generated.js';
import { isCanvasMessage } from '../CanvasValidation.generated.js';
import { isCanvasSurfaceCapabilitiesReply } from '../CanvasSurfaceValidation.generated.js';
import { isCanvasMaskMessage } from '../CanvasMasksValidation.generated.js';
import { isMemoryAllowanceMessage } from '../MemoryValidation.generated.js';
import { isUpdatesHostMessage } from '../UpdatesValidation.generated.js';
import { emptyCanvasSurfaceCapabilities } from '../CanvasSurfaceDiscovery.js';
import { NEWEST_CANVAS_SURFACE_SCHEMA } from '../CanvasSurfaceSchemas.js';

/**
 * The host side of one panel channel, seen from the page.
 *
 * `send` delivers a page message to the host; `subscribe` receives every host message for this
 * channel until released. The adapter forwards values unchanged and grants no other authority.
 */
export interface ConformanceHostTransport {
  send(message: unknown): void;
  subscribe(receive: (value: unknown) => void): () => void;
}

/** Bind a run to the session the host opened its channel with. */
export interface HostConformanceOptions {
  /** The panel session identity the host addresses; 16 to 80 letters, digits or hyphens. */
  readonly session: string;
  /** Ends a run that is still waiting for the host; the run then rejects as cancelled. */
  readonly cancellation?: CanvasCancellationSource;
}

/** One kit check: passed, or not offered because the host does not declare what it needs. */
export interface ConformanceResult {
  readonly check: string;
  readonly outcome: 'passed' | 'not-offered';
}

/** One message as the run saw it, in order, for recording a host profile. */
export interface ConformanceExchange {
  readonly from: 'page' | 'host';
  readonly message: unknown;
}

/** A completed run: the declaration the host made, each check's outcome and the exchange. */
export interface HostConformanceReport {
  readonly description: HostDescriptionMessage;
  readonly results: readonly ConformanceResult[];
  readonly transcript: readonly ConformanceExchange[];
}

/** A host failed one check; `received` holds the offending host message when there is one. */
export class HostConformanceFailure extends Error {
  constructor(
    readonly check: string,
    message: string,
    readonly received?: unknown,
  ) {
    super(`${check}: ${message}`);
    this.name = 'HostConformanceFailure';
  }
}

/** The checks a run makes, in order, under the names results and failures report. */
export const CONFORMANCE_CHECKS = Object.freeze([
  'host-description',
  'panel-presentation',
  'canvas-discovery',
  'canvas-step-down',
  'memory',
  'updates',
  'capability.presentation.host-styles',
  'capability.presentation.system-accent',
  'capability.output.host-surface',
] as const);

type CheckName = (typeof CONFORMANCE_CHECKS)[number];

/** Read a protocol name from a value, when it has one. */
function protocolOf(value: unknown): unknown {
  return value && typeof value === 'object'
    ? (value as { protocol?: unknown }).protocol
    : undefined;
}

/** Whether a host message is valid for its protocol and that protocol may be spoken now. */
function validHostMessage(value: unknown, declared: DeclaredHost | null): boolean {
  switch (protocolOf(value)) {
    case 'sugarcubes.host':
      return isHostDescriptionMessage(value);
    case 'sugarcubes.panel':
      return isPanelConfigure(value) || isPanelPing(value);
    case HOST_PROTOCOLS.canvas.name:
      return !!declared?.serves(HOST_PROTOCOLS.canvas) && isCanvasMessage(value);
    case HOST_PROTOCOLS.canvasMasks.name:
      return !!declared?.serves(HOST_PROTOCOLS.canvasMasks) && isCanvasMaskMessage(value);
    case HOST_PROTOCOLS.memory.name:
      return !!declared?.serves(HOST_PROTOCOLS.memory) && isMemoryAllowanceMessage(value);
    case HOST_PROTOCOLS.updates.name:
      return !!declared?.serves(HOST_PROTOCOLS.updates) && isUpdatesHostMessage(value);
    default:
      return false;
  }
}

/** Address a page message to the session's envelope. */
function envelope<Name extends string, Version extends number>(
  protocol: { readonly name: Name; readonly version: Version },
  session: string,
) {
  return { protocol: protocol.name, version: protocol.version, session } as const;
}

/**
 * Play the page against one host and verify everything it declares.
 *
 * @throws HostConformanceFailure at the first check the host fails, including at once when the
 *   host sends a message its schema rejects, a message for another session, or a protocol it
 *   did not declare.
 */
export async function runHostConformance(
  transport: ConformanceHostTransport,
  options: HostConformanceOptions,
): Promise<HostConformanceReport> {
  const probe = new HostProbe(transport, options);
  try {
    return await conform(probe, options.session);
  } finally {
    probe.close();
  }
}

async function conform(probe: HostProbe, session: string): Promise<HostConformanceReport> {
  const results: ConformanceResult[] = [];
  const pass = (check: CheckName): void => void results.push({ check, outcome: 'passed' });
  const skip = (check: CheckName): void => void results.push({ check, outcome: 'not-offered' });

  probe.check = 'host-description';
  probe.send({ ...envelope({ name: 'sugarcubes.panel', version: 3 }, session), type: 'ready' });
  const description = await probe.next('the host description', isHostDescriptionMessage);
  const declared = DeclaredHost.read(description);
  if (!declared)
    throw new HostConformanceFailure(probe.check, 'repeats a protocol or capability', description);
  if (!declared.serves(HOST_PROTOCOLS.panel))
    throw new HostConformanceFailure(probe.check, 'does not list sugarcubes.panel', description);
  probe.declared = declared;
  pass('host-description');

  probe.check = 'panel-presentation';
  const configure = await probe.next('a configure after the description', isPanelConfigure);
  probe.send({ ...envelope({ name: 'sugarcubes.panel', version: 3 }, session), type: 'applied' });
  await probe.next('a ping after the page applied its presentation', isPanelPing);
  pass('panel-presentation');

  let discovery: CanvasSurfaceCapabilitiesReply | null = null;
  if (declared.serves(HOST_PROTOCOLS.canvas)) {
    probe.check = 'canvas-discovery';
    discovery = await discover(probe, session, 1, NEWEST_CANVAS_SURFACE_SCHEMA);
    pass('canvas-discovery');
    probe.check = 'canvas-step-down';
    const newer = NEWEST_CANVAS_SURFACE_SCHEMA + 1000;
    const stepped = await discover(probe, session, 2, newer);
    if (stepped.status === 'error' || stepped.capabilities.schema >= newer)
      throw new HostConformanceFailure(
        probe.check,
        'a request for an unknown schema must be answered in a schema the host knows',
        stepped,
      );
    pass('canvas-step-down');
  } else {
    skip('canvas-discovery');
    skip('canvas-step-down');
  }

  if (declared.serves(HOST_PROTOCOLS.memory)) {
    probe.check = 'memory';
    const memory = envelope(HOST_PROTOCOLS.memory, session);
    probe.send({ ...memory, type: 'preference', preference: { mode: 'share', percent: 25 } });
    probe.send({ ...memory, type: 'usage', usedBytes: 0, demandBytes: 0, consumers: [] });
    await probe.next('a memory allowance', (value): value is MemoryAllowanceMessage =>
      isMemoryAllowanceMessage(value),
    );
    pass('memory');
  } else skip('memory');

  if (declared.serves(HOST_PROTOCOLS.updates)) {
    probe.check = 'updates';
    const updates = envelope(HOST_PROTOCOLS.updates, session);
    probe.send({ ...updates, type: 'watch' });
    await probe.next(
      'an update state after watch',
      (value): value is UpdatesStateMessage =>
        isUpdatesHostMessage(value) && value.type === 'state',
    );
    probe.send({ ...updates, type: 'check', request: 'conformance-check' });
    await probe.next(
      'an answer naming the check request',
      (value) =>
        isUpdatesHostMessage(value) &&
        (value.type === 'accepted' || value.type === 'rejected') &&
        value.request === 'conformance-check',
    );
    pass('updates');
  } else skip('updates');

  capability(
    results,
    declared,
    'capability.presentation.host-styles',
    HOST_CAPABILITIES.hostStyles,
  );
  probe.check = 'capability.presentation.system-accent';
  if (declared.declares(HOST_CAPABILITIES.systemAccent)) {
    if (!validAccent(configure.presentation))
      throw new HostConformanceFailure(probe.check, 'configure carries no accent color', configure);
    pass('capability.presentation.system-accent');
  } else skip('capability.presentation.system-accent');
  probe.check = 'capability.output.host-surface';
  if (declared.declares(HOST_CAPABILITIES.hostOutputSurface)) {
    if (!discovery || discovery.status === 'unsupported')
      throw new HostConformanceFailure(
        probe.check,
        'a host that presents output must serve canvas surface discovery',
        discovery,
      );
    pass('capability.output.host-surface');
  } else skip('capability.output.host-surface');

  return { description, results, transcript: probe.transcript };
}

/** Record a capability the kit verifies through the presentation the host already sent. */
function capability(
  results: ConformanceResult[],
  declared: DeclaredHost,
  check: CheckName,
  name: (typeof HOST_CAPABILITIES)[keyof typeof HOST_CAPABILITIES],
): void {
  results.push({ check, outcome: declared.declares(name) ? 'passed' : 'not-offered' });
}

/** A system accent host sends its accent; a host whose accent is unavailable sends null. */
function validAccent(presentation: PanelPresentation): boolean {
  return Object.hasOwn(presentation, 'accentColor');
}

/** Ask for surface capabilities in one schema and verify the reply's own consistency. */
async function discover(
  probe: HostProbe,
  session: string,
  requestId: number,
  schema: number,
): Promise<CanvasSurfaceCapabilitiesReply> {
  probe.send({
    ...envelope(HOST_PROTOCOLS.canvas, session),
    requestId,
    type: 'surface.capabilities.request',
    schema,
  });
  const reply = await probe.next(
    `a surface capability reply for request ${String(requestId)}`,
    (value): value is CanvasSurfaceCapabilitiesReply =>
      isCanvasSurfaceCapabilitiesReply(value) && value.requestId === requestId,
  );
  if (reply.capabilities.schema > schema)
    throw new HostConformanceFailure(probe.check, 'answered a newer schema than requested', reply);
  if (reply.status !== 'available' && !emptyCanvasSurfaceCapabilities(reply.capabilities))
    throw new HostConformanceFailure(
      probe.check,
      'declared surface capabilities while the surface is not available',
      reply,
    );
  return reply;
}

/** Collect host messages in order and fail the current check at the first invalid one. */
class HostProbe {
  check: CheckName = 'host-description';
  declared: DeclaredHost | null = null;
  readonly transcript: ConformanceExchange[] = [];
  readonly #queue: unknown[] = [];
  #failure: Error | null = null;
  #wake: (() => void) | null = null;
  readonly #stop: () => void;
  readonly #stopCancellation: () => void;

  constructor(
    private readonly transport: ConformanceHostTransport,
    private readonly options: HostConformanceOptions,
  ) {
    const cancelled = (): void => {
      this.#fail(new Error('The conformance run was cancelled.'));
    };
    this.#stop = transport.subscribe((value) => {
      this.#receive(value);
    });
    this.#stopCancellation = options.cancellation?.subscribe(cancelled) ?? (() => undefined);
    if (options.cancellation?.cancelled()) cancelled();
  }

  send(message: unknown): void {
    this.#throwIfFailed();
    this.transcript.push({ from: 'page', message });
    this.transport.send(message);
  }

  /**
   * Take host messages in order until one matches; a valid message that does not match is
   * passed over, so unrelated host traffic such as change notifications never fails a check.
   */
  async next<Message>(
    expected: string,
    matches: (value: unknown) => value is Message,
  ): Promise<Message>;
  async next(expected: string, matches: (value: unknown) => boolean): Promise<unknown>;
  async next(_expected: string, matches: (value: unknown) => boolean): Promise<unknown> {
    for (;;) {
      this.#throwIfFailed();
      const value = this.#queue.shift();
      if (value !== undefined) {
        if (matches(value)) return value;
        continue;
      }
      await new Promise<void>((resolve) => {
        this.#wake = resolve;
      });
      this.#wake = null;
    }
  }

  close(): void {
    this.#stop();
    this.#stopCancellation();
  }

  #receive(value: unknown): void {
    if (this.#failure) return;
    this.transcript.push({ from: 'host', message: value });
    const session =
      value && typeof value === 'object' ? (value as { session?: unknown }).session : undefined;
    if (session !== this.options.session || !validHostMessage(value, this.declared)) {
      this.#fail(
        new HostConformanceFailure(
          this.check,
          'the host sent a message its schema rejects, for another session, or for a protocol it did not declare',
          value,
        ),
      );
      return;
    }
    this.#queue.push(value);
    this.#wake?.();
  }

  #fail(error: Error): void {
    this.#failure ??= error;
    this.#wake?.();
  }

  #throwIfFailed(): void {
    if (this.#failure) throw this.#failure;
  }
}
