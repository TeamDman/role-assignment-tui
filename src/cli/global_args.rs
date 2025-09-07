use arbitrary::Arbitrary;
use clap::Args;
use std::ffi::OsString;

use crate::cli::to_args::ToArgs;

#[derive(Args, Default, Arbitrary, PartialEq, Debug)]
pub struct GlobalArgs {
    /// Enable debug logging
    #[clap(long, global = true)]
    pub debug: bool,
}

impl GlobalArgs {
    pub fn log_level(&self) -> tracing::Level {
        if self.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        }
    }
}

impl ToArgs for GlobalArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        if self.debug {
            args.push("--debug".into());
        }
        args
    }
}
