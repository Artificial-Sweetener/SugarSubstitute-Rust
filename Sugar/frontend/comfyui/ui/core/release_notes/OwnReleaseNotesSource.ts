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
 * Read SugarCubes' own published release notes from its backend.
 *
 * The backend reads the changelog published at the running version's release tag through its
 * bounded release client; the page never reaches the release repository itself.
 * `GET /sugarcubes/release-notes` names the product and running version, and
 * `GET /sugarcubes/release-notes/notes?since=x.y.z` answers the notes or a failure.
 */
import { isRecord } from '../../types/common.js';
import {
  unavailableNotes,
  type PublishedRelease,
  type ReleaseNotesIdentity,
  type ReleaseNotesOutcome,
  type ReleaseNotesSource,
} from './ReleaseNotesSource.js';

const VERSION_PATH = '/sugarcubes/release-notes';
const NOTES_PATH = '/sugarcubes/release-notes/notes';
const TOKEN = /^[a-z0-9.-]{1,64}$/;

/** One JSON reply from the backend; `body` is the decoded reply or null. */
export interface ReleaseNotesReply {
  readonly status: number;
  readonly body: unknown;
}

/** Reach the release notes backend. */
export interface ReleaseNotesBackend {
  get(path: string): Promise<ReleaseNotesReply>;
}

function decodeIdentity(value: unknown): ReleaseNotesIdentity | null {
  if (!isRecord(value) || value.schema !== 1) return null;
  const { product, currentVersion } = value;
  if (typeof product !== 'string' || product.length === 0 || product.length > 64) return null;
  if (typeof currentVersion !== 'string' || !TOKEN.test(currentVersion)) return null;
  return Object.freeze({ product, currentVersion });
}

function decodeRelease(value: unknown): PublishedRelease | null {
  if (!isRecord(value)) return null;
  const { version, notes } = value;
  if (typeof version !== 'string' || !TOKEN.test(version)) return null;
  if (typeof notes !== 'string' || notes.length > 262144) return null;
  return { version, notes };
}

/** Validate one notes reply; anything unexpected reads as unavailable. */
export function decodeReleaseNotes(value: unknown): ReleaseNotesOutcome {
  if (!isRecord(value) || value.schema !== 1) return unavailableNotes();
  if (value.failure === 'not-published') return { kind: 'failed', reason: 'not-published' };
  if (value.failure !== undefined) return unavailableNotes();
  const { releases, omitted } = value;
  if (!Array.isArray(releases) || releases.length === 0 || releases.length > 64)
    return unavailableNotes();
  if (!Number.isSafeInteger(omitted) || Number(omitted) < 0) return unavailableNotes();
  const decoded = releases.map(decodeRelease);
  if (decoded.some((release) => release === null)) return unavailableNotes();
  return {
    kind: 'notes',
    releases: decoded.filter((release): release is PublishedRelease => release !== null),
    omitted: Number(omitted),
  };
}

/** SugarCubes' own release notes as a {@link ReleaseNotesSource}. */
export class OwnReleaseNotesSource implements ReleaseNotesSource {
  readonly #listeners = new Set<() => void>();
  #identity: ReleaseNotesIdentity | null = null;
  #retired = false;

  constructor(
    private readonly backend: ReleaseNotesBackend,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Read the product and running version once. */
  async start(): Promise<void> {
    let reply: ReleaseNotesReply;
    try {
      reply = await this.backend.get(VERSION_PATH);
    } catch (error: unknown) {
      this.report(error);
      return;
    }
    const identity = reply.status === 200 ? decodeIdentity(reply.body) : null;
    if (this.#retired || !identity) return;
    this.#identity = identity;
    for (const changed of [...this.#listeners]) changed();
  }

  /** Stop announcing; later reads settle as unavailable. */
  retire(): void {
    this.#retired = true;
    this.#listeners.clear();
  }

  read(): ReleaseNotesIdentity | null {
    return this.#identity;
  }

  subscribe(changed: () => void): () => void {
    this.#listeners.add(changed);
    return () => this.#listeners.delete(changed);
  }

  async load(since: string): Promise<ReleaseNotesOutcome> {
    if (this.#retired || !TOKEN.test(since)) return unavailableNotes();
    try {
      const reply = await this.backend.get(`${NOTES_PATH}?since=${encodeURIComponent(since)}`);
      return reply.status === 200 ? decodeReleaseNotes(reply.body) : unavailableNotes();
    } catch (error: unknown) {
      this.report(error);
      return unavailableNotes();
    }
  }
}

/** Adapt ComfyUI's `api.fetchApi` to the backend port, or return null without it. */
export function comfyReleaseNotesBackend(api: unknown): ReleaseNotesBackend | null {
  const comfy = isRecord(api) ? api : null;
  const fetchApi = comfy?.fetchApi;
  if (!comfy || typeof fetchApi !== 'function') return null;
  return {
    async get(path) {
      const response: unknown = await fetchApi.call(comfy, path, { method: 'GET' });
      const reply = isRecord(response) ? response : null;
      const status = reply?.status;
      const json = reply?.json;
      if (typeof status !== 'number' || typeof json !== 'function')
        throw new TypeError('ComfyUI returned an unreadable reply.');
      const body: unknown = await Promise.resolve(json.call(response)).catch(() => null);
      return { status, body };
    },
  };
}
