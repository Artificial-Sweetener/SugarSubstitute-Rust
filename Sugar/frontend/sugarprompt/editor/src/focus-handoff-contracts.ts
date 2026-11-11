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
/** A source-free handoff can restore only its current one-shot native focus intent. */
export interface FocusHandoff {
  /** Consume once; later host intent or a chosen external destination returns false. Spatial edits retain their native scroll anchor with preserve-viewport. */
  restore(viewport?: 'reveal-selection' | 'preserve-viewport'): Promise<boolean>;
  /** Release only this handoff; a newer native intent remains current. */
  cancel(): void;
}
