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
/** Cache validated media-action metadata without polling, image retention or renderer ownership. */
import type { OutputMediaProfile } from './domain/OutputMedia.js';

/** Own profile revision, observation and teardown independently of individual byte transfers. */
export class OutputMediaProfileSource {
  readonly #listeners = new Set<() => void>();
  #profile: OutputMediaProfile = Object.freeze({
    preferredCopyFormat: 'png',
    fileAction: 'download',
    photoshop: false,
  });
  #revision = 0;
  #disposed = false;

  /** Inject the validated capability boundary instead of reading host or saving globals. */
  constructor(private readonly load: () => Promise<OutputMediaProfile>) {}

  /** Read the last accepted metadata with a conservative download endpoint before initialization. */
  read(): OutputMediaProfile {
    return this.#profile;
  }

  /** Observe semantic capability changes without installing timers or native mount observers. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Refuse superseded responses and notify only when the accepted profile actually changes. */
  async refresh(): Promise<void> {
    if (this.#disposed) return;
    const revision = ++this.#revision;
    const profile = await this.load();
    if (this.#disposed || revision !== this.#revision) return;
    if (
      profile.preferredCopyFormat === this.#profile.preferredCopyFormat &&
      profile.fileAction === this.#profile.fileAction &&
      profile.photoshop === this.#profile.photoshop
    )
      return;
    this.#profile = profile;
    for (const listener of [...this.#listeners]) listener();
  }

  /** Invalidate late observations and release every consumer on host teardown. */
  dispose(): void {
    this.#disposed = true;
    this.#revision++;
    this.#listeners.clear();
  }
}
