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
/** Check isolated runtime environments while allowing repository preparation without runtime entry points. */
import path from 'node:path';
import ts from 'typescript';

/** Validate one compiler environment and distinguish an empty scaffold from compiler failure. */
export function checkProject(configPath: string): readonly ts.Diagnostic[] {
  const loaded = ts.readConfigFile(configPath, ts.sys.readFile);
  if (loaded.error) return [loaded.error];
  const config = ts.parseJsonConfigFileContent(loaded.config, ts.sys, path.dirname(configPath));
  // An empty source tree is intentional during repository preparation; every other config error blocks.
  const errors = config.errors.filter((error) => error.code !== 18003);
  if (errors.length || !config.fileNames.length) return errors;
  return ts.getPreEmitDiagnostics(
    ts.createProgram(config.fileNames, { ...config.options, noEmit: true }),
  );
}

const projects = process.argv.slice(2);
if (!projects.length) throw new Error('Provide at least one TypeScript project.');
for (const project of projects) {
  const errors = checkProject(path.resolve(project));
  for (const error of errors)
    console.error(ts.flattenDiagnosticMessageText(error.messageText, '\n'));
  if (errors.length) process.exitCode = 1;
  else console.log(`TypeScript environment valid: ${project}`);
}
