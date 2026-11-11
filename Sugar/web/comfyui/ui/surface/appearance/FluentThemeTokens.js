/**
 * Supply Fluent's input fill ramp, which Microsoft's web token set does not publish.
 *
 * WinUI's ControlFillColor Default/Secondary/InputActive/Disabled values, also used by the
 * qfluentwidgets LineEdit, SpinBox and ComboBox styles: a focused field darkens in the dark theme
 * and becomes solid white in the light theme. The fills are translucent so they compose over any
 * card material.
 */
const fieldFills = {
    dark: {
        'field-fill': 'rgba(255,255,255,0.0605)',
        'field-hover-fill': 'rgba(255,255,255,0.0837)',
        'field-focus-fill': 'rgba(30,30,30,0.7)',
        'field-disabled-fill': 'rgba(255,255,255,0.0419)',
    },
    light: {
        'field-fill': 'rgba(255,255,255,0.7)',
        'field-hover-fill': 'rgba(249,249,249,0.5)',
        'field-focus-fill': '#ffffff',
        'field-disabled-fill': 'rgba(249,249,249,0.3)',
    },
};
/** Require every presentation role to be supplied by Microsoft's trusted theme for one scheme. */
export function fluentThemeTokens(theme, dark) {
    return {
        text: String(theme.colorNeutralForeground1 ?? ''),
        'text-secondary': String(theme.colorNeutralForeground2 ?? ''),
        'text-muted': String(theme.colorNeutralForeground3 ?? ''),
        'text-subtle': String(theme.colorNeutralForeground4 ?? ''),
        'text-disabled': String(theme.colorNeutralForegroundDisabled ?? ''),
        'text-on-accent': String(theme.colorNeutralForegroundOnBrand ?? ''),
        surface: String(theme.colorNeutralBackground1 ?? ''),
        'surface-hover': String(theme.colorNeutralBackground1Hover ?? ''),
        'surface-pressed': String(theme.colorNeutralBackground1Pressed ?? ''),
        'surface-secondary': String(theme.colorNeutralBackground2 ?? ''),
        'surface-tertiary': String(theme.colorNeutralBackground3 ?? ''),
        'surface-disabled': String(theme.colorNeutralBackgroundDisabled ?? ''),
        border: String(theme.colorNeutralStroke1 ?? ''),
        'border-hover': String(theme.colorNeutralStroke1Hover ?? ''),
        'border-subtle': String(theme.colorNeutralStroke2 ?? ''),
        'border-accessible': String(theme.colorNeutralStrokeAccessible ?? ''),
        'border-accessible-hover': String(theme.colorNeutralStrokeAccessibleHover ?? ''),
        'border-disabled': String(theme.colorNeutralStrokeDisabled ?? ''),
        'brand-fill': String(theme.colorBrandBackground ?? ''),
        'brand-fill-hover': String(theme.colorBrandBackgroundHover ?? ''),
        'brand-fill-pressed': String(theme.colorBrandBackgroundPressed ?? ''),
        'brand-text': String(theme.colorBrandForeground1 ?? ''),
        'brand-border': String(theme.colorBrandStroke1 ?? ''),
        'brand-indicator': String(theme.colorCompoundBrandBackground ?? ''),
        'brand-indicator-hover': String(theme.colorCompoundBrandBackgroundHover ?? ''),
        'danger-text': String(theme.colorPaletteRedForeground1 ?? ''),
        'focus-ring': String(theme.colorStrokeFocus2 ?? ''),
        'subtle-hover': String(theme.colorSubtleBackgroundHover ?? ''),
        'subtle-selected': String(theme.colorSubtleBackgroundSelected ?? ''),
        'font-family': String(theme.fontFamilyBase ?? ''),
        'font-size': String(theme.fontSizeBase300 ?? ''),
        'line-height': String(theme.lineHeightBase300 ?? ''),
        'font-weight': String(theme.fontWeightRegular ?? ''),
        'font-weight-strong': String(theme.fontWeightSemibold ?? ''),
        'control-radius': String(theme.borderRadiusMedium ?? ''),
        'field-radius': 'var(--sc-control-radius)',
        'field-border-width': '1px',
        ...fieldFills[dark ? 'dark' : 'light'],
        'field-stroke': 'var(--sc-border)',
        'field-bottom-stroke': 'var(--sc-border-accessible)',
        'field-hover-stroke': 'var(--sc-border-hover)',
        'field-hover-bottom-stroke': 'var(--sc-border-accessible-hover)',
        'field-focus-stroke': 'var(--sc-border)',
        'field-focus-bottom-stroke': 'var(--sc-brand-border)',
        'field-focus-shadow': 'inset 0 -1px var(--sc-brand-border)',
        'field-focus-outline': 'none',
        'field-focus-outline-offset': '0px',
        'field-underline-display': 'block',
        'button-radius': 'var(--sc-control-radius)',
        'button-border-width': '1px',
        'button-bottom-stroke': 'var(--sc-border-accessible)',
        'button-fill': 'var(--sc-surface)',
        'button-hover-fill': 'var(--sc-surface-hover)',
        'button-pressed-fill': 'var(--sc-surface-pressed)',
        'shadow-small': String(theme.shadow4 ?? ''),
        'shadow-medium': String(theme.shadow16 ?? ''),
        'shadow-large': String(theme.shadow64 ?? ''),
        'motion-faster': String(theme.durationFaster ?? ''),
        'motion-fast': String(theme.durationFast ?? ''),
        'motion-duration': String(theme.durationNormal ?? ''),
        'motion-ease': String(theme.curveEasyEase ?? ''),
        'motion-decelerate': String(theme.curveDecelerateMid ?? ''),
    };
}
