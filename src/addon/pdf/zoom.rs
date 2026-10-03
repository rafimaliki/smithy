//! Zoom for the PDF viewer: fit the pages to the viewport width, or a fixed factor.
//! Pure so the arithmetic is tested without a window.

/// The smallest and largest scale a page may draw at.
pub const MIN: f32 = 0.1;
pub const MAX: f32 = 6.0;

/// One zoom step: a click on `+` or `-` moves by this much.
pub const STEP: f32 = 1.25;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Zoom {
    /// The widest page fills the viewport width.
    Fit,
    Fixed(f32),
}

impl Zoom {
    /// The scale pages draw at. `viewport` is the pages area's width in pixels and
    /// `widest` the widest page's width in points; both are zero before the first layout,
    /// when there is nothing to fit to yet.
    pub fn scale(self, viewport: f32, widest: f32) -> f32 {
        match self {
            Zoom::Fixed(scale) => scale.clamp(MIN, MAX),
            Zoom::Fit if viewport > 0.0 && widest > 0.0 => (viewport / widest).clamp(MIN, MAX),
            Zoom::Fit => 1.0,
        }
    }

    /// The next zoom after a step of `factor` (1.25 in, 0.8 out) from the scale on
    /// screen, so a step out of Fit moves one visible step rather than a whole factor.
    pub fn stepped(self, scale: f32, factor: f32) -> Zoom {
        let next = (scale * factor).clamp(MIN, MAX);
        Zoom::Fixed((next * 1000.0).round() / 1000.0)
    }

    /// The toolbar label: `Fit`, or the factor as a percentage.
    pub fn label(self, scale: f32) -> String {
        match self {
            Zoom::Fit => "Fit".to_string(),
            Zoom::Fixed(_) => format!("{}%", (scale * 100.0).round() as i32),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_divides_the_viewport_by_the_widest_page() {
        assert_eq!(Zoom::Fit.scale(600.0, 800.0), 0.75);
        // Nothing has been laid out yet: the pages draw at their natural size.
        assert_eq!(Zoom::Fit.scale(0.0, 800.0), 1.0);
    }

    #[test]
    fn fixed_clamps_to_the_limits() {
        assert_eq!(Zoom::Fixed(9.0).scale(600.0, 800.0), MAX);
        assert_eq!(Zoom::Fixed(0.001).scale(600.0, 800.0), MIN);
    }

    #[test]
    fn a_step_out_of_fit_starts_at_the_scale_it_produced() {
        // Fit produced 0.8; one step out of that is 0.8 / 1.25, not 1.0 / 1.25.
        assert_eq!(Zoom::Fit.stepped(0.8, 1.0 / STEP), Zoom::Fixed(0.64));
        assert_eq!(Zoom::Fixed(1.0).stepped(1.0, STEP), Zoom::Fixed(1.25));
    }

    #[test]
    fn labels_are_fit_or_a_percentage() {
        assert_eq!(Zoom::Fit.label(0.73), "Fit");
        assert_eq!(Zoom::Fixed(1.25).label(1.25), "125%");
    }
}
