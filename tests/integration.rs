//! Cross-module regression tests for configuration, model labels and the model catalog contract.

#[path = "integration/config_migration.rs"]
mod config_migration;

#[path = "integration/model_display.rs"]
mod model_display;

#[path = "integration/model_contract.rs"]
mod model_contract;
