mod director;
mod palette;
mod renderer;
mod state;

pub(crate) use director::SceneDirector;
pub(crate) use palette::{palette_for, smooth_palette};
pub use renderer::{
    prepare_renderer_surface, probe_renderer, run_renderer, RendererLifecycle, RendererStatus,
};
pub(crate) use state::{intensity_ceiling, intensity_values, FlashEnvelope};
pub use state::{IntensityProfile, PaletteName, SmoothedVisualState, VisualSettings, VisualStyle};
