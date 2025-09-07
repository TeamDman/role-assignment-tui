pub mod list_resource_groups;
pub mod resource_group_tui;

use crate::cli::command::list_resource_groups::ListResourceGroupsArgs;
use crate::cli::command::resource_group_tui::ResourceGroupTuiArgs;
use crate::cli::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Subcommand;
use std::ffi::OsString;

/// A demonstration command line utility
#[derive(Subcommand, Arbitrary, PartialEq, Debug)]
pub enum Command {
    /// List Azure resource groups
    ListResourceGroups(ListResourceGroupsArgs),
    /// Launch a TUI for resource groups (stub)
    ResourceGroupTui(ResourceGroupTuiArgs),
}

impl Command {
    pub fn invoke(self) -> eyre::Result<()> {
        match self {
            Command::ListResourceGroups(args) => args.invoke(),
            Command::ResourceGroupTui(args) => args.invoke(),
        }
    }
}

impl ToArgs for Command {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match self {
            Command::ListResourceGroups(lrg_args) => {
                args.push("list-resource-groups".into());
                args.extend(lrg_args.to_args());
            }
            Command::ResourceGroupTui(rg_tui_args) => {
                args.push("resource-group-tui".into());
                args.extend(rg_tui_args.to_args());
            }
        }
        args
    }
}
