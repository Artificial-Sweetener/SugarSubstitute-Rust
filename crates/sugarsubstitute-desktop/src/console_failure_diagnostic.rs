// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed observational console failure facts shared by the desktop writer and host decoder.
//! Values carry neither native handles nor lifecycle authority. Parsers retain no input bytes.

/// Define authored labels and canonical wire tokens from one reviewed operation inventory.
macro_rules! operations {
    ($($variant:ident => $native:literal),+ $(,)?) => {
        /// The exact authored console operation that returned the first retained failure.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum ConsoleFailureOperation {
            $(#[doc = concat!("The authored `", $native, "` operation.")] $variant),+
        }
        impl ConsoleFailureOperation {
            /// Parse one exact canonical wire token; unknown or extended tokens are rejected.
            /// The returned value owns no raw input and grants no native authority.
            pub fn parse(value: &str) -> Option<Self> {
                Some(match value { $(stringify!($variant) => Self::$variant,)+ _ => return None })
            }
            /// Match only implemented static owner labels before projecting diagnostic metadata.
            pub(crate) fn from_operation(value: &str) -> Option<Self> {
                Some(match value { $($native => Self::$variant,)+ _ => return None })
            }
        }
        #[cfg(test)]
        mod operation_contract_tests {
            use super::ConsoleFailureOperation as Operation;
            #[test]
            fn every_authored_console_operation_roundtrips_its_canonical_debug_token() {
                $(
                    let actual = Operation::from_operation($native);
                    assert_eq!(actual, Some(Operation::$variant));
                    assert_eq!(Operation::parse(&format!("{:?}", Operation::$variant)), actual);
                    assert_eq!(Operation::parse($native), None);
                )+
            }
        }
    };
}
operations! {
    Accessibility => "console_accessibility",
    AccessibilityClaim => "console_accessibility_claim",
    AccessibilityDisconnect => "console_accessibility_disconnect",
    AccessibilityQuery => "console_accessibility_query",
    Activate => "console_activate",
    AlphaEndDraw => "console_alpha_end_draw",
    AlphaPresent => "console_alpha_present",
    AlphaReadback => "console_alpha_readback",
    BeginPaint => "console_begin_paint",
    ClipboardRender => "console_clipboard_render",
    ColorGlyphFactory => "console_color_glyph_factory",
    CompositeExtent => "console_composite_extent",
    Deactivate => "console_deactivate",
    EndPaint => "console_end_paint",
    Engine => "console_engine",
    Factory => "console_factory",
    FocusActivate => "console_focus_activate",
    FocusDeactivate => "console_focus_deactivate",
    FrameCrop => "console_frame_crop",
    GlyphClaim => "console_glyph_claim",
    GlyphDpi => "console_glyph_dpi",
    GlyphsRetired => "console_glyphs_retired",
    Host => "console_host",
    HostBounds => "console_host_bounds",
    HostDcIdentity => "console_host_dc_identity",
    HostDcLimit => "console_host_dc_limit",
    HostFailed => "console_host_failed",
    HostFocus => "console_host_focus",
    HostGetDc => "console_host_get_dc",
    HostImeRelease => "console_host_ime_release",
    HostInvalidate => "console_host_invalidate",
    HostReleaseDc => "console_host_release_dc",
    HostRetirement => "console_host_retirement",
    HostScrollAxis => "console_host_scroll_axis",
    HostScrollRange => "console_host_scroll_range",
    HostTimerAdmission => "console_host_timer_admission",
    HostTimerInstall => "console_host_timer_install",
    HostTimerRemove => "console_host_timer_remove",
    HostTimerRetire => "console_host_timer_retire",
    InputIdentity => "console_input_identity",
    InputInstall => "console_input_install",
    InputPlatform => "console_input_platform",
    InterfaceIdentity => "console_interface_identity",
    Layout => "console_layout",
    LibraryIdentity => "console_library_identity",
    Message => "console_message",
    NativeAlphaDraw => "console_native_alpha_draw",
    NativeDelivery => "console_native_delivery",
    NativeNotification => "console_native_notification",
    NativePremultiplication => "console_native_premultiplication",
    OutputRetired => "console_output_retired",
    PaintAllocate => "console_paint_allocate",
    PaintCapacity => "console_paint_capacity",
    PaintClaim => "console_paint_claim",
    PaintDocument => "console_paint_document",
    PaintFormat => "console_paint_format",
    PaintFunding => "console_paint_funding",
    PaintMessage => "console_paint_message",
    PaintPalette => "console_paint_palette",
    PaintRange => "console_paint_range",
    PaintReentry => "console_paint_reentry",
    PaintRestore => "console_paint_restore",
    PaintRestoreFormat => "console_paint_restore_format",
    PaintRestoreInput => "console_paint_restore_input",
    PaintTokens => "console_paint_tokens",
    PaintVerify => "console_paint_verify",
    Renderer => "console_renderer",
    Scale => "console_scale",
    ScrollAnimationRetire => "console_scroll_animation_retire",
    ScrollBrush => "console_scroll_brush",
    ScrollCapture => "console_scroll_capture",
    ScrollCaptureRetire => "console_scroll_capture_retire",
    ScrollFont => "console_scroll_font",
    ScrollFontAlignment => "console_scroll_font_alignment",
    ScrollFontFactory => "console_scroll_font_factory",
    ScrollGeometry => "console_scroll_geometry",
    ScrollTimer => "console_scroll_timer",
    ScrollTimerRetire => "console_scroll_timer_retire",
    ScrollTrack => "console_scroll_track",
    SelectionStyle => "console_selection_style",
    SemanticPaint => "console_semantic_paint",
    Services => "console_services",
    Shutdown => "console_shutdown",
    ShutdownReference => "console_shutdown_reference",
    SystemTheme => "console_system_theme",
    TextCapture => "console_text_capture",
    TextCaptureRelease => "console_text_capture_release",
    UiDeactivate => "console_ui_deactivate",
    WicBitmap => "console_wic_bitmap",
    WicFactory => "console_wic_factory",
    WicTarget => "console_wic_target",
    AccessibilityClear => "console_accessibility_clear",
    AccessibilityCreate => "console_accessibility_create",
    AccessibilityLabel => "console_accessibility_label",
    AccessibilityName => "console_accessibility_name",
    Adopt => "console_adopt",
    Availability => "console_availability",
    Bind => "console_bind",
    BoundsVisibility => "console_bounds_visibility",
    Claim => "console_claim",
    CreateBounds => "console_create_bounds",
    FirstLine => "console_first_line",
    HighContrast => "console_high_contrast",
    Identity => "console_identity",
    IndexAllocate => "console_index_allocate",
    IndexValidate => "console_index_validate",
    LibraryFree => "text_library_free",
    LibraryLoad => "text_library_load",
    LineFromCharacter => "console_line_from_character",
    LineIndex => "console_line_index",
    LinksDisable => "console_links_disable",
    NativeObservation => "console_native_observation",
    Palette => "console_palette",
    Present => "console_present",
    Publish => "console_publish",
    Readonly => "console_readonly",
    Register => "console_register",
    Repaint => "console_repaint",
    ScrollLines => "console_scroll_lines",
    ScrollObserve => "console_scroll_observe",
    ScrollRestore => "console_scroll_restore",
    SelectionObserve => "console_selection_observe",
    SelectionRestore => "console_selection_restore",
    Stage => "console_stage",
    StylesAllocate => "console_styles_allocate",
    TextAllocate => "console_text_allocate",
    TextFormat => "console_text_format",
    TextReadback => "console_text_readback",
    TextReadbackAllocate => "console_text_readback_allocate",
    TextStream => "console_text_stream",
    WindowCreate => "console_window_create",
    WindowDestroy => "console_window_destroy",
}

/// Closed categories for a directly retained IO error without a native error number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleFailureIoKind {
    /// The direct IO operation timed out.
    TimedOut,
    /// The direct IO endpoint disconnected.
    Disconnected,
    /// The direct IO operation was denied.
    Denied,
    /// The direct IO input or data was invalid.
    InvalidInput,
    /// Another direct IO category was returned.
    Io,
}
/// Inert facts from supported direct concrete causes, without message text or source traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleFailureCode {
    /// Direct Windows HRESULT, represented as its signed 32-bit value.
    Hresult(i32),
    /// Direct operating-system error number, represented as a signed 32-bit value.
    Os(i32),
    /// A directly retained IO kind without a native error number.
    Io(ConsoleFailureIoKind),
    /// A checked native integer conversion failed.
    IntegerRange,
    /// Bounded console storage allocation was rejected.
    Allocation,
    /// The admitted immutable frame could not fund the console allocation.
    MemoryClaim,
    /// An exact 24-bit native color was absent from the current semantic token table.
    UnknownPaintToken(u32),
    /// A native brush did not contain a normalized opaque semantic token.
    InvalidPaintColor,
    /// RichEdit requested an unsupported render-target operation.
    UnsupportedPaint,
    /// A native glyph advertised image formats not supported by the current renderer.
    UnsupportedGlyphFormat(i32),
    /// The direct original cause has no supported concrete projection.
    Opaque,
}
impl ConsoleFailureCode {
    /// Parse exactly one canonical direct code; reject unknown categories and extensions.
    /// Native integers use signed decimal i32 without plus signs, leading zeroes or negative zero.
    /// No allocation, error callbacks or input-byte retention occurs.
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "Opaque" => Self::Opaque,
            "IntegerRange" => Self::IntegerRange,
            "Allocation" => Self::Allocation,
            "MemoryClaim" => Self::MemoryClaim,
            "InvalidPaintColor" => Self::InvalidPaintColor,
            "UnsupportedPaint" => Self::UnsupportedPaint,
            "Io(TimedOut)" => Self::Io(ConsoleFailureIoKind::TimedOut),
            "Io(Disconnected)" => Self::Io(ConsoleFailureIoKind::Disconnected),
            "Io(Denied)" => Self::Io(ConsoleFailureIoKind::Denied),
            "Io(InvalidInput)" => Self::Io(ConsoleFailureIoKind::InvalidInput),
            "Io(Io)" => Self::Io(ConsoleFailureIoKind::Io),
            _ => {
                if let Some(raw) =
                    value.strip_prefix("UnknownPaintToken(").and_then(|v| v.strip_suffix(')'))
                {
                    return native(raw)
                        .filter(|value| (0..=0x00ff_ffff).contains(value))
                        .map(|value| Self::UnknownPaintToken(value as u32));
                }
                if let Some(raw) =
                    value.strip_prefix("UnsupportedGlyphFormat(").and_then(|v| v.strip_suffix(')'))
                {
                    return native(raw).map(Self::UnsupportedGlyphFormat);
                }
                if let Some(raw) = value.strip_prefix("Hresult(").and_then(|v| v.strip_suffix(')'))
                {
                    return native(raw).map(Self::Hresult);
                }
                if let Some(raw) = value.strip_prefix("Os(").and_then(|v| v.strip_suffix(')')) {
                    return native(raw).map(Self::Os);
                }
                return None;
            }
        })
    }
}
/// Validate canonical decimal syntax before the bounded native-number conversion.
fn native(value: &str) -> Option<i32> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits.starts_with('0') && digits.len() > 1)
        || value == "-0"
    {
        return None;
    }
    value.parse().ok()
}

#[cfg(test)]
mod code_contract_tests {
    use super::{ConsoleFailureCode as Code, ConsoleFailureIoKind as Kind};
    #[test]
    fn direct_console_codes_roundtrip_canonical_debug_and_reject_extended_or_noncanonical_tokens() {
        for code in [
            Code::Opaque,
            Code::Allocation,
            Code::IntegerRange,
            Code::MemoryClaim,
            Code::UnknownPaintToken(0),
            Code::UnknownPaintToken(0x00ff_ffff),
            Code::InvalidPaintColor,
            Code::UnsupportedPaint,
            Code::UnsupportedGlyphFormat(128),
            Code::Io(Kind::Io),
            Code::Io(Kind::TimedOut),
            Code::Io(Kind::Disconnected),
            Code::Io(Kind::Denied),
            Code::Io(Kind::InvalidInput),
            Code::Os(i32::MIN),
            Code::Os(0),
            Code::Os(i32::MAX),
            Code::Hresult(i32::MIN),
            Code::Hresult(i32::MAX),
        ] {
            assert_eq!(Code::parse(&format!("{code:?}")), Some(code));
        }
        for bad in [
            "PRIVATE",
            "Io(Other)",
            "Io(PRIVATE)",
            "Os(+1)",
            "Os(-0)",
            "Os(01)",
            "Os(2147483648)",
            "Hresult(-2147483649)",
            "Hresult(0x80070005)",
            "Hresult(1))",
            "Opaque PRIVATE",
            "MemoryClaim(1)",
            "UnknownPaintToken(-1)",
            "UnknownPaintToken(16777216)",
            "UnknownPaintToken(01)",
            "UnsupportedGlyphFormat(+1)",
            "Allocation\nPRIVATE",
            "",
        ] {
            assert_eq!(Code::parse(bad), None);
        }
    }
}
