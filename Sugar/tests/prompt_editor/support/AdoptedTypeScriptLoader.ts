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
/** Resolve emitted-style relative imports for native Node TypeScript contracts without replacing source loader hooks. */
import { registerHooks } from 'node:module';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

registerHooks({
  /** Preserve Node's module-typescript format so export-only test observations retain native type stripping. */
  resolve(specifier, context, next) {
    if (
      (specifier.startsWith('.') || specifier.startsWith('file:')) &&
      /\.js(?:\?|$)/.test(specifier)
    ) {
      const candidate = new URL(specifier.replace(/\.js(?=\?|$)/, '.ts'), context.parentURL);
      if (existsSync(fileURLToPath(candidate))) return next(candidate.href, context);
    }
    return next(specifier, context);
  },
});
