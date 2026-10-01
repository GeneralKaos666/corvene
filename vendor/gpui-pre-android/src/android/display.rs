//! Android display handling.
//!
//! The NDK gives no display list at the C level: geometry comes from the
//! `ANativeWindow` the system hands over and density from `AConfiguration`.
//! One `AndroidDisplay` describes the surface the activity draws into, so in
//! a freeform window (DeX, Chromebooks) it is the window, not the monitor.
//!
//! Ported from gpui-mobile's `src/android/display.rs`.

use anyhow::Result;
use gpui::{point, px, size, Bounds, DevicePixels, DisplayId, Pixels, PlatformDisplay, Size};

/// Android's baseline density: one logical pixel per device pixel at 160 dpi.
pub(crate) const DENSITY_DEFAULT: f32 = 160.0;

#[derive(Clone, Debug)]
pub struct AndroidDisplay {
    size: Size<DevicePixels>,
    scale_factor: f32,
}

impl AndroidDisplay {
    pub(crate) fn new(size: Size<DevicePixels>, scale_factor: f32) -> Self {
        Self { size, scale_factor }
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }
}

/// The scale factor of a density in dots per inch as `AConfiguration` reports
/// it; unknown and special values fall back to 1.
pub(crate) fn scale_factor_for_density(density_dpi: Option<u32>) -> f32 {
    match density_dpi {
        // ACONFIGURATION_DENSITY_ANY / _NONE are 0xfffe / 0xffff
        Some(dpi) if dpi > 0 && dpi < 0xfffe => dpi as f32 / DENSITY_DEFAULT,
        _ => 1.0,
    }
}

impl PlatformDisplay for AndroidDisplay {
    fn id(&self) -> DisplayId {
        DisplayId::new(0)
    }

    fn uuid(&self) -> Result<uuid::Uuid> {
        Ok(uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_OID,
            b"android-display-0",
        ))
    }

    fn bounds(&self) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(
                px(self.size.width.0 as f32 / self.scale_factor),
                px(self.size.height.0 as f32 / self.scale_factor),
            ),
        }
    }
}
