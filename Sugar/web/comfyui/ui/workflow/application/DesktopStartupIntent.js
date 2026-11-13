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
/** Protect startup recovery from settings-only interruption and later explicit authored intent. */
export class DesktopStartupIntent {
    #acknowledged = false;
    #settingsInterrupted = false;
    #restoreSuperseded = false;
    #committedContext = null;
    #userLoad = false;
    /** Seed the first complete normal Comfy workspace; startup completion itself is not user intent. */
    committed(context) {
        this.#committedContext ??= context;
    }
    /** Observe actual public workflow/load transitions, excluding the controlled materialization transaction. */
    workflow(context, loading, controlled) {
        if (controlled || this.#committedContext === null)
            return;
        if (loading) {
            this.#userLoad = true;
            return;
        }
        if (this.#userLoad || context !== this.#committedContext)
            this.authored();
        this.#userLoad = false;
        this.#committedContext = context;
    }
    /** Settings alone suspends startup autosave even after the dialog closes. */
    settingsOpened() {
        if (!this.#acknowledged)
            this.#settingsInterrupted = true;
    }
    /** A real graph/workflow/generation intent owns a fresh capture frontier and supersedes startup restore. */
    authored() {
        const changed = !this.#restoreSuperseded || this.#settingsInterrupted;
        this.#restoreSuperseded = true;
        this.#settingsInterrupted = false;
        return changed;
    }
    /** Adopt the controlled restored context without treating its fresh runtime bindings as user edits. */
    restored(context) {
        this.#committedContext = context;
        this.#userLoad = false;
    }
    /** Release startup-only protection once the host confirms the complete workspace durable or restored. */
    acknowledged() {
        this.#acknowledged = true;
        this.#settingsInterrupted = false;
    }
    /** New authored work may be captured while original recovery bytes remain host-protected. */
    captureAllowed() {
        return !this.#settingsInterrupted;
    }
    /** Refuse retargeting startup restore over any subsequently authored workspace. */
    restoreAllowed() {
        return !this.#settingsInterrupted && !this.#restoreSuperseded;
    }
}
