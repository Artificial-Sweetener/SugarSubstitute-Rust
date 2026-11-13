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
/** Prove the native build selects exactly one declared VS2022 installation and names each refusal. */
import { declaredInstallation } from '../../../scripts/gimp/NativeEnvironment.js';

const product = 'Microsoft.VisualStudio.Product.Enterprise';
const enterprise = 'C:\\Program Files\\Microsoft Visual Studio\\2022\\Enterprise';

test('selects the single installation vswhere reports with Windows line endings', () => {
  expect(declaredInstallation(product, `${enterprise}\r\n`)).toBe(enterprise);
});

test('reports a missing declared product as absent rather than ambiguous', () => {
  for (const report of ['', '\r\n', '   \r\n'])
    expect(() => declaredInstallation(product, report)).toThrow(
      'VS2022 (Microsoft.VisualStudio.Product.Enterprise) is not installed.',
    );
});

test('refuses to choose between several installations of the declared product', () => {
  expect(() =>
    declaredInstallation(product, `${enterprise}\r\nD:\\VS\\2022\\Enterprise\r\n`),
  ).toThrow('Select exactly one installation of the declared VS2022 product; 2 were found.');
});

test('refuses a path the developer-shell script cannot quote', () => {
  expect(() => declaredInstallation(product, "C:\\Visual Studio's\\2022\r\n")).toThrow(
    'cannot be passed to the developer shell',
  );
});
