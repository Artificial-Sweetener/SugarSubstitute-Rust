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
 * Record what a host release answers, and replay every recording against the current page code.
 *
 * Each host release records one profile with {@link recordHostProfile} from a passing
 * conformance run and stores it as `profiles/<host>/<release>.json` beside this module. Older
 * profiles stay, so a page change that an older host release can no longer use fails replay.
 */
import { HOST_PROTOCOLS } from '../../contracts/HostDescription.js';
import type { CanvasSurfaceHost } from '../../contracts/CanvasSurface.js';
import { DeclaredHost } from '../HostDescription.js';
import { PanelClientSession } from '../PanelSession.js';
import { isHostDescriptionMessage } from '../HostValidation.generated.js';
import { isPanelConfigure, isPanelPing } from '../PanelValidation.generated.js';
import { isCanvasMessage } from '../CanvasValidation.generated.js';
import { isCanvasSurfaceCapabilitiesReply } from '../CanvasSurfaceValidation.generated.js';
import { isCanvasMaskMessage } from '../CanvasMasksValidation.generated.js';
import { isMemoryAllowanceMessage } from '../MemoryValidation.generated.js';
import { isUpdatesHostMessage } from '../UpdatesValidation.generated.js';
import { discoverCanvasSurface } from '../CanvasSurfaceDiscovery.js';
import type { ConformanceExchange, HostConformanceReport } from './HostConformance.js';

/** Every host SugarCubes keeps a recorded profile for. */
export const HOST_PROFILE_SLOTS = Object.freeze([
  'comfyui',
  'gimp',
  'photoshop',
  'substitute',
] as const);

/** One host SugarCubes keeps a profile for. */
export type HostProfileSlot = (typeof HOST_PROFILE_SLOTS)[number];

/**
 * Hosts whose profile is not recorded yet, with who records it.
 *
 * A slot listed here has no fixture; recording its profile means removing it from this list.
 */
export const PENDING_HOST_PROFILES: Readonly<Partial<Record<HostProfileSlot, string>>> =
  Object.freeze({
    substitute:
      'SugarSubstitute records its profile from its Rust host with this kit and adds it as profiles/substitute/<release>.json.',
  });

/** A host release's recorded answers; `none` is a page with no host channel, such as plain ComfyUI. */
export interface RecordedHostProfile {
  readonly schema: 1;
  readonly host: HostProfileSlot;
  readonly release: string;
  readonly channel: 'panel' | 'none';
  readonly session?: string;
  readonly transcript: readonly ConformanceExchange[];
}

/** Turn a passing conformance run into the profile a host release stores. */
export function recordHostProfile(
  host: HostProfileSlot,
  release: string,
  session: string,
  report: HostConformanceReport,
): RecordedHostProfile {
  return { schema: 1, host, release, channel: 'panel', session, transcript: report.transcript };
}

/** The profile of a page that runs with no host channel. */
export function recordStandaloneProfile(
  host: HostProfileSlot,
  release: string,
): RecordedHostProfile {
  return { schema: 1, host, release, channel: 'none', transcript: [] };
}

/** Why a recorded profile no longer works with the current page code. */
export class HostProfileReplayFailure extends Error {
  constructor(
    readonly profile: string,
    message: string,
    readonly received?: unknown,
  ) {
    super(`${profile}: ${message}`);
    this.name = 'HostProfileReplayFailure';
  }
}

/**
 * Feed a recorded host's messages to the page's readers and return what the page settles on.
 *
 * The description must settle a declaration; the configure must be applied; every surface
 * discovery reply must be read without an invalid-reply failure; every other message must pass
 * the current guard for a protocol the host declared.
 *
 * @throws HostProfileReplayFailure at the first recorded message the current page cannot use.
 */
export async function replayHostProfile(profile: RecordedHostProfile): Promise<DeclaredHost> {
  const name = `${profile.host} ${profile.release}`;
  if (profile.channel === 'none') {
    if (profile.transcript.length)
      throw new HostProfileReplayFailure(name, 'a page without a host channel exchanges nothing');
    return DeclaredHost.standalone;
  }
  const session = profile.session ?? '';
  let declared: DeclaredHost | null = null;
  const page = { applied: false };
  const sent: unknown[] = [];
  const client = new PanelClientSession(session, {
    send: (message) => sent.push(message),
    apply: () => {
      page.applied = true;
      return true;
    },
    connected: () => true,
    report: (error) => {
      throw new HostProfileReplayFailure(name, 'the page failed to apply the presentation', error);
    },
  });
  for (const { from, message } of profile.transcript) {
    if (from !== 'host') continue;
    const fail = (reason: string): never => {
      throw new HostProfileReplayFailure(name, reason, message);
    };
    if (isHostDescriptionMessage(message)) {
      if (message.session !== session) fail('the description names another session');
      declared ??= DeclaredHost.read(message) ?? fail('the description does not settle');
    } else if (isPanelConfigure(message)) {
      if (!declared) fail('configure arrived before the description');
      if (!client.receive(message) || !page.applied) fail('the page did not apply the configure');
    } else if (isCanvasSurfaceCapabilitiesReply(message)) {
      if (!declared?.serves(HOST_PROTOCOLS.canvas)) fail('canvas reply from a host without canvas');
      await replayDiscovery(name, message);
    } else if (!declaredMessage(message, declared)) fail('the current page rejects this message');
  }
  if (!declared) throw new HostProfileReplayFailure(name, 'the profile holds no host description');
  if (!page.applied) throw new HostProfileReplayFailure(name, 'the profile holds no configure');
  return declared;
}

/** Whether a recorded message passes the current guard for a protocol the host declared. */
function declaredMessage(message: unknown, declared: DeclaredHost | null): boolean {
  if (!declared) return false;
  return (
    isPanelPing(message) ||
    (isCanvasMessage(message) && declared.serves(HOST_PROTOCOLS.canvas)) ||
    (isCanvasMaskMessage(message) && declared.serves(HOST_PROTOCOLS.canvasMasks)) ||
    (isMemoryAllowanceMessage(message) && declared.serves(HOST_PROTOCOLS.memory)) ||
    (isUpdatesHostMessage(message) && declared.serves(HOST_PROTOCOLS.updates))
  );
}

/** Read one recorded surface discovery reply through the page's own discovery. */
async function replayDiscovery(name: string, reply: unknown): Promise<CanvasSurfaceHost | null> {
  const failures: unknown[] = [];
  let surface: CanvasSurfaceHost | null;
  try {
    surface = await discoverCanvasSurface({
      read: () => Promise.resolve(reply as never),
      current: () => true,
      owns: () => false,
      execute: () => Promise.reject(new Error('A replayed profile executes no surface commands.')),
      report: (error) => failures.push(error),
    });
  } catch (error: unknown) {
    throw new HostProfileReplayFailure(name, 'the page refuses this discovery reply', error);
  }
  // The page reports an error status or an inconsistent reply; a recorded release must have neither.
  if (failures.length)
    throw new HostProfileReplayFailure(name, 'the page reported this discovery reply', failures);
  return surface;
}
