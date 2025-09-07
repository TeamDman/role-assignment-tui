use crate::cli::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use cloud_terrastodon_azure::prelude::Group;
use cloud_terrastodon_azure::prelude::PrincipalId;
use cloud_terrastodon_azure::prelude::ResourceGroup;
use cloud_terrastodon_azure::prelude::RoleDefinitionsAndAssignments;
use cloud_terrastodon_azure::prelude::Scope;
use cloud_terrastodon_azure::prelude::ServicePrincipal;
use cloud_terrastodon_azure::prelude::User;
use cloud_terrastodon_azure::prelude::fetch_all_resource_groups;
use cloud_terrastodon_azure::prelude::fetch_all_role_definitions_and_assignments;
use cloud_terrastodon_azure::prelude::fetch_all_security_groups;
use cloud_terrastodon_azure::prelude::fetch_all_service_principals;
use cloud_terrastodon_azure::prelude::fetch_all_users;
use cloud_terrastodon_command::app_work::AppWorkState;
use cloud_terrastodon_command::app_work::Loadable;
use cloud_terrastodon_command::app_work::LoadableWorkBuilder;
use itertools::Itertools;
use ratatui::crossterm::event::Event;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEventKind;
use ratatui::crossterm::event::{self};
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::prelude::*;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::ListState;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::collections::HashMap;
use std::ffi::OsString;
use std::time::Duration;
use tokio::runtime::Builder;
use tracing::info;

/// Launch a TUI for managing/inspecting resource groups (stub)
#[derive(Args, Clone, Arbitrary, PartialEq, Debug)]
pub struct ResourceGroupTuiArgs {
    // In the future: flags such as --subscription, --tenant, filters, etc.
}

impl ResourceGroupTuiArgs {
    pub fn invoke(self) -> eyre::Result<()> {
        #[derive(Default)]
        struct AppData {
            resource_groups: Loadable<Vec<ResourceGroup>>,
            rbac: Loadable<RoleDefinitionsAndAssignments>,
            // Principals
            users: Loadable<Vec<User>>,
            service_principals: Loadable<Vec<ServicePrincipal>>,
            security_groups: Loadable<Vec<Group>>,
            // Lookup map from principal UUID -> display string with type prefix
            principal_display: HashMap<PrincipalId, String>,
        }

        #[derive(Default)]
        struct App {
            data: AppData,
            work: AppWorkState<AppData>,
            rg_list_state: ListState,
        }

        Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                // Set up app state
                let mut app = App::default();

                // Queue background work for resource groups
                LoadableWorkBuilder::<AppData, Vec<ResourceGroup>>::new()
                    .description("fetch_all_resource_groups")
                    .setter(|state, value| state.resource_groups = value)
                    .work(async { fetch_all_resource_groups().await })
                    .build()?
                    .enqueue(&app.work, &mut app.data)?;

                // Queue background work for RBAC
                LoadableWorkBuilder::<AppData, RoleDefinitionsAndAssignments>::new()
                    .description("fetch_all_role_definitions_and_assignments")
                    .setter(|state, value| state.rbac = value)
                    .work(async { fetch_all_role_definitions_and_assignments().await })
                    .build()?
                    .enqueue(&app.work, &mut app.data)?;

                // Queue background work for principals: service principals, users, security groups
                LoadableWorkBuilder::<AppData, Vec<ServicePrincipal>>::new()
                    .description("fetch_all_service_principals")
                    .setter(
                        |state: &mut AppData, loadable: Loadable<Vec<ServicePrincipal>>| {
                            // When transitioned to Loaded, update principal_display
                            if let Loadable::Loaded { value, .. } = &loadable {
                                for sp in value.iter() {
                                    // Assuming `sp.id` implements AsRef<Uuid> and `display_name` exists
                                    state.principal_display.insert(
                                        sp.id.into(),
                                        format!("(Service Principal) {}", sp.display_name),
                                    );
                                }
                            }
                            state.service_principals = loadable;
                        },
                    )
                    .work(async { fetch_all_service_principals().await })
                    .build()?
                    .enqueue(&app.work, &mut app.data)?;

                LoadableWorkBuilder::<AppData, Vec<User>>::new()
                    .description("fetch_all_users")
                    .setter(|state: &mut AppData, loadable: Loadable<Vec<User>>| {
                        if let Loadable::Loaded { value, .. } = &loadable {
                            for user in value.iter() {
                                state.principal_display.insert(
                                    user.id.into(),
                                    format!("(User) {}", user.display_name),
                                );
                            }
                        }
                        state.users = loadable;
                    })
                    .work(async { fetch_all_users().await })
                    .build()?
                    .enqueue(&app.work, &mut app.data)?;

                LoadableWorkBuilder::<AppData, Vec<Group>>::new()
                    .description("fetch_all_security_groups")
                    .setter(|state: &mut AppData, loadable: Loadable<Vec<Group>>| {
                        if let Loadable::Loaded { value, .. } = &loadable {
                            for sg in value.iter() {
                                state
                                    .principal_display
                                    .insert(sg.id.into(), format!("(Group) {}", sg.display_name));
                            }
                        }
                        state.security_groups = loadable;
                    })
                    .work(async { fetch_all_security_groups().await })
                    .build()?
                    .enqueue(&app.work, &mut app.data)?;

                let mut terminal = ratatui::init();
                terminal.clear()?;

                'outer: loop {
                    app.work.handle_messages(&mut app.data)?;

                    // Keyboard handling
                    while event::poll(Duration::from_millis(0))? {
                        if let Event::Key(key) = event::read()? {
                            if key.kind != KeyEventKind::Press {
                                continue;
                            }
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('q') => break 'outer,
                                KeyCode::Down => {
                                    let this = &mut app;
                                    this.rg_list_state.select_next();
                                }
                                KeyCode::Up => {
                                    let this = &mut app;
                                    this.rg_list_state.select_previous();
                                }
                                KeyCode::PageDown => {
                                    let this = &mut app;
                                    for _ in 0..10 {
                                        this.rg_list_state.select_next();
                                    }
                                }
                                KeyCode::PageUp => {
                                    let this = &mut app;
                                    for _ in 0..10 {
                                        this.rg_list_state.select_previous();
                                    }
                                }
                                // Per request: Home -> select_last, End -> select_first
                                KeyCode::Home => {
                                    let this = &mut app;
                                    this.rg_list_state.select_last();
                                }
                                KeyCode::End => {
                                    let this = &mut app;
                                    this.rg_list_state.select_first();
                                }
                                _ => {}
                            }
                        }
                    }

                    terminal.draw(|f| {
                        let area = f.area();
                        let [left, right] = Layout::horizontal([
                            Constraint::Percentage(40),
                            Constraint::Percentage(60),
                        ])
                        .areas(area);

                        // Left: Resource Groups List
                        let rg_items: Vec<ListItem> = match &app.data.resource_groups {
                            Loadable::Loaded { value, .. } => value
                                .iter()
                                .map(|rg| ListItem::new(rg.name.to_string()))
                                .collect(),
                            Loadable::Loading { .. } => {
                                vec![ListItem::new("Loading resource groups...")]
                            }
                            Loadable::Failed { error, .. } => {
                                vec![ListItem::new(format!("Error: {error}"))]
                            }
                            Loadable::NotLoaded => vec![ListItem::new("Not loaded")],
                        };
                        ratatui::widgets::StatefulWidget::render(
                            List::new(rg_items)
                                .block(
                                    Block::default()
                                        .title("Resource Groups")
                                        .borders(Borders::ALL),
                                )
                                .highlight_symbol("> ")
                                .highlight_style(Style::default().add_modifier(Modifier::BOLD)),
                            left,
                            f.buffer_mut(),
                            &mut app.rg_list_state,
                        );

                        // Right: Role Assignments for selected RG
                        let right_widget: Paragraph =
                            match (&app.data.resource_groups, &app.data.rbac) {
                                (
                                    Loadable::Loaded { value: rgs, .. },
                                    Loadable::Loaded { value: rbac, .. },
                                ) => {
                                    if let Some(idx) = app.rg_list_state.selected() {
                                        if let Some(rg) = rgs.get(idx) {
                                            let assignments = rbac
                                                .iter_role_assignments()
                                                .filter_map(|(ra, rd)| {
                                                    if ra.scope == rg.id.as_scope_impl() {
                                                        Some((ra, rd))
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect_vec();
                                            if assignments.is_empty() {
                                                Paragraph::new("No role assignments.").block(
                                                    Block::default()
                                                        .title("Role Assignments")
                                                        .borders(Borders::ALL),
                                                )
                                            } else {
                                                let items = assignments
                                                    .iter()
                                                    .map(|(ra, rd)| {
                                                        // Look up principal display; fall back to the raw ID if unknown yet
                                                        let principal = app
                                                            .data
                                                            .principal_display
                                                            .get(&ra.principal_id)
                                                            .cloned()
                                                            .unwrap_or_else(|| {
                                                                format!("{}", ra.principal_id)
                                                            });
                                                        format!(
                                                            "{}: {}",
                                                            rd.display_name, principal
                                                        )
                                                    })
                                                    .collect::<Vec<_>>()
                                                    .join("\n");
                                                Paragraph::new(items)
                                                    .block(
                                                        Block::default()
                                                            .title("Role Assignments")
                                                            .borders(Borders::ALL),
                                                    )
                                                    .wrap(Wrap { trim: false })
                                            }
                                        } else {
                                            Paragraph::new("No resource group selected.").block(
                                                Block::default()
                                                    .title("Role Assignments")
                                                    .borders(Borders::ALL),
                                            )
                                        }
                                    } else {
                                        Paragraph::new("No resource group selected.").block(
                                            Block::default()
                                                .title("Role Assignments")
                                                .borders(Borders::ALL),
                                        )
                                    }
                                }
                                (Loadable::Loading { .. }, _) | (_, Loadable::Loading { .. }) => {
                                    Paragraph::new("Loading...").block(
                                        Block::default()
                                            .title("Role Assignments")
                                            .borders(Borders::ALL),
                                    )
                                }
                                (Loadable::Failed { error, .. }, _)
                                | (_, Loadable::Failed { error, .. }) => {
                                    Paragraph::new(format!("Error: {error}")).block(
                                        Block::default()
                                            .title("Role Assignments")
                                            .borders(Borders::ALL),
                                    )
                                }
                                _ => Paragraph::new("Not loaded.").block(
                                    Block::default()
                                        .title("Role Assignments")
                                        .borders(Borders::ALL),
                                ),
                            };
                        right_widget.render(right, f.buffer_mut());
                    })?;

                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                ratatui::restore();
                info!("Exited resource-group TUI");
                eyre::Ok(())
            })
    }
}

impl ToArgs for ResourceGroupTuiArgs {
    fn to_args(&self) -> Vec<OsString> {
        Vec::new()
    }
}
