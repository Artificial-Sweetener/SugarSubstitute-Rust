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
/** Retain bounded, lightweight inspection state across output-tab attachment lifetimes. */
export interface CanvasVideoSession {
  readonly time: number;
  readonly volume: number;
  readonly muted: boolean;
  readonly loop: boolean;
}

/** Retain only transient state; decoders and GPU resources belong to the visible attachment. */
export class BrowserCanvasMediaSessions {
  readonly #videos = new Map<string, CanvasVideoSession>();
  readonly #models = new Map<string, unknown>();

  /** Read an exact output revision's playback position and preferences. */
  video(key: string): CanvasVideoSession | undefined {
    return this.#videos.get(key);
  }
  /** Remember bounded playback state without retaining the media element. */
  rememberVideo(key: string, state: CanvasVideoSession): void {
    this.#set(this.#videos, key, state);
  }
  /** Read host camera state belonging to one output revision. */
  model(key: string): unknown {
    return this.#models.get(key);
  }
  /** Remember the host's camera snapshot without retaining its renderer. */
  rememberModel(key: string, state: unknown): void {
    this.#set(this.#models, key, state);
  }
  /** Release all session data at host teardown. */
  dispose(): void {
    this.#videos.clear();
    this.#models.clear();
  }
  /** Bound inspection history even when a workflow produces many output revisions. */
  #set<Value>(entries: Map<string, Value>, key: string, value: Value): void {
    entries.delete(key);
    entries.set(key, value);
    while (entries.size > 64) {
      const oldest = entries.keys().next().value;
      if (oldest !== undefined) entries.delete(oldest);
    }
  }
}
