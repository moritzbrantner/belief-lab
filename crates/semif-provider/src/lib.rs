mod error;
mod model;
mod model_files;
mod output;
mod process;
mod provider;
mod setup;

pub use error::SemifSetupError;
pub use model::{model_catalog, SemifModelPin, SemifModelTier};
pub use model_files::{download_model, model_is_ready, model_path};
pub use provider::{SemifBackend, SemifProvider};
pub use setup::{
    bootstrap_semif, prepare_tokenizer, semif_install_is_ready, semif_score_path,
    SemifInstallBackend, SemifSetupOutcome,
};

pub const SEMIF_REPOSITORY: &str = "https://github.com/TheoLeeCJ/SemIf.git";
pub const SEMIF_SOURCE_REVISION: &str = "1f2dea3e25379f9dfc98cb83c324f00ab5deda37";
