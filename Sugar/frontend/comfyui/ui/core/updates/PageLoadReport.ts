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
 * Tell SugarCubes' backend whether this page loaded SugarCubes, so a just-installed version that
 * cannot load its page is put back at the next ComfyUI start.
 *
 * The report runs from a separate, tiny ComfyUI extension: if the main SugarCubes module fails to
 * load at all, it never registers, and this one can still say so. ComfyUI loads every extension
 * module before it runs any extension's setup, so a missing registration at setup time is a load
 * failure. A registered extension announces how its own setup ended on the document.
 */

/** The ComfyUI extension name of SugarCubes' main frontend. */
export const SUGARCUBES_EXTENSION_NAME = 'SugarCubes.UI';
/** The document event the main extension dispatches when its setup has ended. */
export const SETUP_SETTLED_EVENT = 'sugarcubes-setup-settled';
/** The document attribute that keeps the setup outcome for a listener that arrives late. */
export const SETUP_OUTCOME_ATTRIBUTE = 'data-sugarcubes-setup';

/** Record how the main extension's setup ended, for the page-load report. */
export function announceSetupOutcome(document: Document, loaded: boolean): void {
  document.documentElement.setAttribute(SETUP_OUTCOME_ATTRIBUTE, loaded ? 'loaded' : 'failed');
  document.dispatchEvent(new CustomEvent(SETUP_SETTLED_EVENT, { detail: { loaded } }));
}

/** Wrap the main extension's setup so it announces how it ended, then rethrows failures. */
export function announcedSetup(
  document: Document,
  setup: () => Promise<void>,
): () => Promise<void> {
  return async () => {
    try {
      await setup();
    } catch (error) {
      announceSetupOutcome(document, false);
      throw error;
    }
    announceSetupOutcome(document, true);
  };
}

function outcomeOf(value: unknown): boolean | null {
  if (typeof value !== 'object' || value === null) return null;
  const loaded: unknown = Reflect.get(value, 'loaded');
  return typeof loaded === 'boolean' ? loaded : null;
}

/**
 * Report once whether the main extension loaded and finished its setup.
 *
 * @returns The release of the pending listener, when the outcome is not known yet.
 */
export function reportPageLoad(options: {
  readonly registeredExtensions: readonly string[];
  readonly document: Document;
  readonly report: (loaded: boolean) => void;
}): () => void {
  if (!options.registeredExtensions.includes(SUGARCUBES_EXTENSION_NAME)) {
    options.report(false);
    return () => {};
  }
  const recorded = options.document.documentElement.getAttribute(SETUP_OUTCOME_ATTRIBUTE);
  if (recorded === 'loaded' || recorded === 'failed') {
    options.report(recorded === 'loaded');
    return () => {};
  }
  const settled = (event: Event): void => {
    const loaded = event instanceof CustomEvent ? outcomeOf(event.detail) : null;
    if (loaded === null) return;
    options.document.removeEventListener(SETUP_SETTLED_EVENT, settled);
    options.report(loaded);
  };
  options.document.addEventListener(SETUP_SETTLED_EVENT, settled);
  return () => options.document.removeEventListener(SETUP_SETTLED_EVENT, settled);
}
