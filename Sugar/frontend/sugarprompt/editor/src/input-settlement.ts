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
/** The sole input owner exposes native readiness and finite command settlement without another caret or input buffer. */
import { Facet } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';

/** A finite command reservation suspends deferred typing until admitted execution or cancellation. */
export interface CommandInputSettlement {
  /** Finish accepted typing at the admitted command boundary before source publication; keyboard numeric acceptance settles synchronously, and geometry-dependent commands settle after measurement. */
  finish(): void;
  /** Release the reservation so the input owner can resume ordinary deferred preparation. */
  cancel(): void;
}

/** The sole native input owner exposes observations and a command boundary without sharing its edit descriptor. */
export interface NativeInputSettlement {
  /** Observe the input owner's real composition-start event before CodeMirror's first document mutation. */
  hasCompositionInput(view: EditorView): boolean;
  /** Retire deferred semantic work when native composition cancellation restores its original exact source. */
  cancelCompositionInput(view: EditorView): void;
  /** Observe the current physical authoring-key lifetime. */
  hasHeldKey(view: EditorView): boolean;
  /** Suspend existing deferred input preparation for one finite source command. */
  reserveCommand(view: EditorView): CommandInputSettlement;
}

/** Standalone states have no input work to reserve or release. */
const inactiveCommand: CommandInputSettlement = Object.freeze({
  /** No accepted input is pending. */
  finish() {},
  /** No resource is retained. */
  cancel() {},
});
/** Standalone state has no mounted input owner. */
const inactiveInput: NativeInputSettlement = Object.freeze({
  /** Standalone states have no observed native input lifetime. */
  hasCompositionInput: () => false,
  /** No native composition descriptor is retained. */
  cancelCompositionInput() {},
  /** No physical key belongs to an unmounted state. */
  hasHeldKey: () => false,
  /** Return the inert finite boundary for state-only consumers. */
  reserveCommand: () => inactiveCommand,
});

/** One input owner supplies readiness and command settlement through supported view-plugin lookup. */
export const nativeInputSettlement = Facet.define<NativeInputSettlement, NativeInputSettlement>({
  /** Reject competing observers that would disagree about native input settlement. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple native input settlement owners');
    return values[0] ?? inactiveInput;
  },
});

/** Public commands and observations share the native platform start boundary, including source-neutral observer transactions before CodeMirror marks its first change. */
export function nativeCompositionStarted(view: EditorView): boolean {
  return (
    view.compositionStarted || view.state.facet(nativeInputSettlement).hasCompositionInput(view)
  );
}
