use anyhow::Result;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use turbo_rcstr::RcStr;
use turbo_tasks::{Vc, trace::TraceRawVcs};

pub(crate) struct DefaultFallbackFont {
    pub name: RcStr,
    pub capsize_key: RcStr,
    pub az_avg_width: f64,
    pub units_per_em: u32,
}

// From https://github.com/vercel/next.js/blob/a3893bf69c83fb08e88c87bf8a21d987a0448c8e/packages/font/src/utils.ts#L4
pub(crate) static DEFAULT_SANS_SERIF_FONT: Lazy<DefaultFallbackFont> =
    Lazy::new(|| DefaultFallbackFont {
        name: "Arial".into(),
        capsize_key: "arial".into(),
        az_avg_width: 934.5116279069767,
        units_per_em: 2048,
    });

pub(crate) static DEFAULT_SERIF_FONT: Lazy<DefaultFallbackFont> =
    Lazy::new(|| DefaultFallbackFont {
        name: "Times New Roman".into(),
        capsize_key: "timesNewRoman".into(),
        az_avg_width: 854.3953488372093,
        units_per_em: 2048,
    });

/// An automatically generated fallback font generated by next/font.
#[turbo_tasks::value(shared)]
pub(crate) struct AutomaticFontFallback {
    /// e.g. `__Roboto_Fallback_c123b8`
    pub scoped_font_family: Vc<RcStr>,
    /// The name of font locally, used in `src: local("{}")`
    pub local_font_family: Vc<RcStr>,
    pub adjustment: Option<FontAdjustment>,
}

#[turbo_tasks::value(shared)]
pub(crate) enum FontFallback {
    /// An automatically generated fallback font generated by next/font. May
    /// include an optional [[FontAdjustment]].
    Automatic(AutomaticFontFallback),
    /// There was an issue preparing the font fallback. Since resolving the
    /// font css cannot fail, proper Errors cannot be returned. Emit an issue,
    /// return this and omit fallback information instead.
    Error,
    /// A list of manually provided font names to use a fallback, as-is.
    Manual(Vec<RcStr>),
}

#[turbo_tasks::value_impl]
impl FontFallback {
    #[turbo_tasks::function]
    pub(crate) fn has_size_adjust(&self) -> Vc<bool> {
        Vc::cell(matches!(self, FontFallback::Automatic(auto) if auto.adjustment.is_some()))
    }
}

#[turbo_tasks::value(transparent)]
pub(crate) struct FontFallbacks(Vec<Vc<FontFallback>>);

#[turbo_tasks::value_impl]
impl FontFallbacks {
    #[turbo_tasks::function]
    pub(crate) async fn has_size_adjust(&self) -> Result<Vc<bool>> {
        for fallback in &self.0 {
            if *fallback.has_size_adjust().await? {
                return Ok(Vc::cell(true));
            }
        }

        Ok(Vc::cell(false))
    }
}

/// An adjustment to be made to a fallback font to approximate the geometry of
/// the main webfont. Rendered as e.g. `ascent-override: 56.8%;` in the
/// stylesheet
#[derive(Debug, PartialEq, Serialize, Deserialize, TraceRawVcs)]
pub(crate) struct FontAdjustment {
    pub ascent: f64,
    pub descent: f64,
    pub line_gap: f64,
    pub size_adjust: f64,
}

// Necessary since floating points in this struct don't implement Eq, but it's
// required for turbo tasks values.
impl Eq for FontAdjustment {}
