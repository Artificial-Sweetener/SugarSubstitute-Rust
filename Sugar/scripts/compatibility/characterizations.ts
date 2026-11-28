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
/** Read local Jest declarations, including parameterized tests, from the files that import a host adapter. */
import ts from 'typescript';
import { readFileSync } from 'node:fs';

const files: string[] = JSON.parse(readFileSync(0, 'utf8'));
const characterization: Record<string, { title: string; line: number }[]> = {};
for (const file of files) {
  const source = ts.createSourceFile(
    file,
    readFileSync(file, 'utf8'),
    ts.ScriptTarget.Latest,
    true,
  );
  const titles: { title: string; line: number }[] = [];
  const visit = (node: ts.Node): void => {
    if (ts.isCallExpression(node)) {
      let callee = node.expression;
      if (ts.isCallExpression(callee)) callee = callee.expression;
      else if (ts.isTaggedTemplateExpression(callee)) callee = callee.tag;
      const name = callee.getText(source);
      const title = node.arguments[0];
      if (
        /^(?:test|it)(?:\.each)?$/.test(name) &&
        title &&
        (ts.isStringLiteralLike(title) || ts.isTemplateExpression(title))
      ) {
        titles.push({
          title: ts.isTemplateExpression(title) ? title.getText(source) : title.text,
          line: source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1,
        });
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  characterization[file] = titles;
}
process.stdout.write(JSON.stringify(characterization));
