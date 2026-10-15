/** Let the containing settings context choose native or Fluent control tokens. */
export function createSettingsCheckbox(document) {
    const input = document.createElement('input');
    input.type = 'checkbox';
    input.className = 'sc-settings-checkbox';
    return input;
}
