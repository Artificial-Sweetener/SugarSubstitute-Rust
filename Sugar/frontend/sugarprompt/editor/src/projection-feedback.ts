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
/** Canonical feedback consumes existing source-current numeric intent through prepared widget roles. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState } from '@codemirror/state';
import type { PreparedProjection } from './projection-contracts.js';
import type { WeightSubject } from './weight-target.js';
import { weightSession } from './weight-session.js';
import { wheelFeedback } from './wheel-feedback.js';
import { decorateWeightFeedback, sameWeightSubject } from './weight-feedback-decoration.js';
/** At most two current/prior subjects delegate boundary-only widget updates; source, input and feedback lifetimes remain unchanged. */
export function projectWeightFeedback(
  prepared: PreparedProjection,
  state: EditorState,
): PreparedProjection {
  const wheel = state.field(wheelFeedback, false)?.target;
  const session = state.field(weightSession, false);
  const active: WeightSubject | null =
    session && (session.kind === 'emphasis' || session.owner === 'pointer')
      ? { kind: session.kind, range: session.range }
      : null;
  const ready: WeightSubject | null = wheel ? { kind: wheel.kind, range: wheel.range } : null;
  const accented: WeightSubject[] = [];
  if (active) accented.push(active);
  if (ready && !accented.some((subject) => sameWeightSubject(subject, ready))) accented.push(ready);
  let all = prepared.all,
    decorations = prepared.decorations;
  for (const subject of [...prepared.accented, ...accented]) {
    const adjusting = !!active && sameWeightSubject(active, subject);
    const readyNow = !!ready && sameWeightSubject(ready, subject);
    const updated = decorateWeightFeedback(all, subject, readyNow, adjusting);
    decorations =
      decorations === all
        ? updated
        : decorateWeightFeedback(decorations, subject, readyNow, adjusting);
    all = updated;
  }
  if (
    all === prepared.all &&
    decorations === prepared.decorations &&
    accented.length === prepared.accented.length &&
    accented.every((subject, index) =>
      sameWeightSubject(subject, requireValue(prepared.accented[index], 'projection-feedback')),
    )
  )
    return prepared;
  return { ...prepared, all, decorations, accented };
}
