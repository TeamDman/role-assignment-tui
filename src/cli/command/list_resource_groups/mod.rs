use crate::cli::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use cloud_terrastodon_azure::prelude::RolePermissionAction;
use cloud_terrastodon_azure::prelude::Scope;
use cloud_terrastodon_azure::prelude::fetch_all_resource_groups;
use cloud_terrastodon_azure::prelude::fetch_all_role_definitions_and_assignments;
use itertools::Itertools;
use serde_json::json;
use std::ffi::OsString;
use tokio::runtime::Builder;
use tokio::try_join;

/// List Azure resource groups
#[derive(Args, Clone, Arbitrary, PartialEq, Debug)]
pub struct ListResourceGroupsArgs {
    // In the future: add flags like --subscription, --tenant, etc.
}

impl ListResourceGroupsArgs {
    pub fn invoke(self) -> eyre::Result<()> {
        Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                let (resource_groups, rbac) = try_join!(
                    fetch_all_resource_groups(),
                    fetch_all_role_definitions_and_assignments()
                )?;
                let mut rtn = Vec::new();
                for rg in resource_groups {
                    let role_assignments_for_rg = rbac
                        .iter_role_assignments()
                        .filter_map(|(ra, rd)| {
                            if ra.scope == rg.id.as_scope_impl() {
                                Some((ra, rd))
                            } else {
                                None
                            }
                        })
                        .map(|(ra, rd)| {
                            let read_perm = RolePermissionAction::new("Microsoft.General/read");
                            let x = rd.satisfies(&[read_perm], &[]);
                            (ra, rd,x)
                        })
                        .collect_vec();
                    rtn.push(json!({
                        "resource_group": rg,
                        "role_assignments": role_assignments_for_rg.iter().map(|(ra, rd, can_read)| {
                            json!({
                                "role_assignment": ra,
                                "role_definition": rd,
                                "can_read": can_read
                            })
                        }).collect::<Vec<_>>(),
                    }));
                }

                let json = serde_json::to_string_pretty(&rtn)?;
                println!("{}", json);
                eyre::Ok(())
            })
    }
}

impl ToArgs for ListResourceGroupsArgs {
    fn to_args(&self) -> Vec<OsString> {
        Vec::new()
    }
}
