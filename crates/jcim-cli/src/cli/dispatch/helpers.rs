use std::path::Path;

use jcim_config::project::find_project_manifest;
use jcim_core::apdu::CommandApdu;
use jcim_sdk::iso7816::SecureMessagingProtocol;
use jcim_sdk::{
    Aid, JcimClient, ProjectRef, ReaderRef, SimulationRef, SimulationSummary, globalplatform,
};

use super::super::args::{
    GpCardStateArg, GpOccurrenceArg, GpRegistryKindArg, GpTransitionArg, ProjectSelectorArgs,
};

/// Resolve CLI project selector arguments into the canonical SDK project selector.
pub(super) fn resolve_project_ref(args: ProjectSelectorArgs) -> Result<ProjectRef, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    resolve_project_ref_from_cwd(args, &cwd)
}

/// Resolve the current working directory into a project selector when no explicit selector was given.
fn resolve_project_ref_from_cwd(
    args: ProjectSelectorArgs,
    cwd: &Path,
) -> Result<ProjectRef, String> {
    if let Some(project_id) = args.id {
        return Ok(ProjectRef::from_id(project_id));
    }
    if let Some(project) = args.project {
        return Ok(ProjectRef::from_path(project));
    }
    if find_project_manifest(cwd).is_some() {
        Ok(ProjectRef::from_path(cwd))
    } else {
        Err(
            "no project selected; pass `--project`, `--id`, or run inside a JCIM project"
                .to_string(),
        )
    }
}

/// Resolve an optional simulation id into one concrete running simulation selector.
pub(super) async fn resolve_simulation_ref(
    client: &JcimClient,
    requested: Option<String>,
) -> Result<SimulationRef, String> {
    if let Some(simulation_id) = requested {
        return Ok(SimulationRef::new(simulation_id));
    }
    let simulations = client
        .list_simulations()
        .await
        .map_err(|error| error.to_string())?;
    resolve_simulation_ref_from_summaries(simulations.as_slice())
}

/// Choose one simulation selector from the currently known simulation summaries.
fn resolve_simulation_ref_from_summaries(
    simulations: &[SimulationSummary],
) -> Result<SimulationRef, String> {
    match simulations {
        [simulation] => Ok(simulation.simulation_ref()),
        [] => Err("no active simulations are available".to_string()),
        _ => Err("multiple simulations are active; pass `--simulation`".to_string()),
    }
}

/// Build one SDK reader selector from an optional CLI reader name.
pub(super) fn reader_ref(reader: Option<String>) -> ReaderRef {
    match reader {
        Some(reader) => ReaderRef::named(reader),
        None => ReaderRef::Default,
    }
}

/// Parse one CLI APDU hex string into the typed command APDU model.
pub(super) fn parse_command_apdu(apdu_hex: &str) -> Result<CommandApdu, String> {
    let bytes = hex::decode(apdu_hex).map_err(|error| error.to_string())?;
    CommandApdu::parse(&bytes).map_err(|error| error.to_string())
}

/// Parse one CLI AID string into the typed AID model.
pub(super) fn parse_aid(aid: &str) -> Result<Aid, String> {
    Aid::from_hex(aid).map_err(|error| error.to_string())
}

/// Parse one CLI secure-messaging protocol token into the typed ISO protocol model.
pub(super) fn parse_secure_messaging_protocol(
    value: &str,
) -> Result<SecureMessagingProtocol, String> {
    let normalized = value.trim();
    match normalized.to_ascii_lowercase().as_str() {
        "iso7816" | "iso-7816" | "iso_7816" => Ok(SecureMessagingProtocol::Iso7816),
        "scp02" => Ok(SecureMessagingProtocol::Scp02),
        "scp03" => Ok(SecureMessagingProtocol::Scp03),
        _ => normalized
            .strip_prefix("other:")
            .or_else(|| normalized.strip_prefix("OTHER:"))
            .map(|label| SecureMessagingProtocol::Other(label.to_string()))
            .ok_or_else(|| {
                "unsupported secure messaging protocol; use `iso7816`, `scp02`, `scp03`, or `other:<label>`".to_string()
            }),
    }
}

/// Map the CLI GP registry-kind enum to the maintained domain enum.
pub(super) fn gp_registry_kind(value: GpRegistryKindArg) -> globalplatform::RegistryKind {
    match value {
        GpRegistryKindArg::Isd => globalplatform::RegistryKind::IssuerSecurityDomain,
        GpRegistryKindArg::Applications => globalplatform::RegistryKind::Applications,
        GpRegistryKindArg::LoadFiles => globalplatform::RegistryKind::ExecutableLoadFiles,
        GpRegistryKindArg::LoadFilesAndModules => {
            globalplatform::RegistryKind::ExecutableLoadFilesAndModules
        }
    }
}

/// Map the CLI GP occurrence enum to the maintained domain enum.
pub(super) fn gp_occurrence(value: GpOccurrenceArg) -> globalplatform::GetStatusOccurrence {
    match value {
        GpOccurrenceArg::FirstOrAll => globalplatform::GetStatusOccurrence::FirstOrAll,
        GpOccurrenceArg::Next => globalplatform::GetStatusOccurrence::Next,
    }
}

/// Map the CLI GP card-state enum to the maintained domain enum.
pub(super) fn gp_card_state(value: GpCardStateArg) -> globalplatform::CardLifeCycle {
    match value {
        GpCardStateArg::OpReady => globalplatform::CardLifeCycle::OpReady,
        GpCardStateArg::Initialized => globalplatform::CardLifeCycle::Initialized,
        GpCardStateArg::Secured => globalplatform::CardLifeCycle::Secured,
        GpCardStateArg::CardLocked => globalplatform::CardLifeCycle::CardLocked,
        GpCardStateArg::Terminated => globalplatform::CardLifeCycle::Terminated,
    }
}

/// Map the CLI GP lock-transition enum to the maintained domain enum.
pub(super) fn gp_transition(value: GpTransitionArg) -> globalplatform::LockTransition {
    match value {
        GpTransitionArg::Lock => globalplatform::LockTransition::Lock,
        GpTransitionArg::Unlock => globalplatform::LockTransition::Unlock,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jcim_core::iso7816::{IsoCapabilities, IsoSessionState};
    use jcim_sdk::SimulationStatus;

    use super::*;

    fn sample_simulation(simulation_id: &str) -> SimulationSummary {
        SimulationSummary {
            simulation_id: simulation_id.to_string(),
            project_id: "project-1".to_string(),
            project_path: PathBuf::from("/tmp/project"),
            status: SimulationStatus::Running,
            reader_name: "Reader".to_string(),
            health: "ready".to_string(),
            atr: None,
            active_protocol: None,
            iso_capabilities: IsoCapabilities::default(),
            session_state: IsoSessionState::default(),
            package_count: 1,
            applet_count: 1,
            package_name: "com.jcim.demo".to_string(),
            package_aid: "F000000001".to_string(),
            recent_events: Vec::new(),
        }
    }

    #[test]
    fn resolve_project_ref_prefers_id_then_path_then_manifest_directory() {
        let cwd = PathBuf::from("/tmp/jcim-cli-dispatch");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        std::fs::write(cwd.join("jcim.toml"), "[project]\nname = 'demo'\n")
            .expect("write manifest");

        assert_eq!(
            resolve_project_ref_from_cwd(
                ProjectSelectorArgs {
                    id: Some("project-id".to_string()),
                    project: Some(PathBuf::from("/tmp/other")),
                },
                &cwd,
            )
            .expect("resolve by id"),
            ProjectRef::from_id("project-id")
        );
        assert_eq!(
            resolve_project_ref_from_cwd(
                ProjectSelectorArgs {
                    id: None,
                    project: Some(PathBuf::from("/tmp/project")),
                },
                &cwd,
            )
            .expect("resolve by path"),
            ProjectRef::from_path("/tmp/project")
        );
        assert_eq!(
            resolve_project_ref_from_cwd(
                ProjectSelectorArgs {
                    id: None,
                    project: None,
                },
                &cwd,
            )
            .expect("resolve by cwd"),
            ProjectRef::from_path(&cwd)
        );

        let _ = std::fs::remove_dir_all(cwd);
    }

    #[test]
    fn resolve_project_ref_reports_missing_selection_without_manifest() {
        let cwd = PathBuf::from("/tmp/jcim-cli-dispatch-missing");
        std::fs::create_dir_all(&cwd).expect("create cwd");

        let error = resolve_project_ref_from_cwd(
            ProjectSelectorArgs {
                id: None,
                project: None,
            },
            &cwd,
        )
        .expect_err("missing project selection should fail");
        assert!(error.contains("no project selected"));

        let _ = std::fs::remove_dir_all(cwd);
    }

    #[test]
    fn resolve_simulation_ref_from_summaries_handles_zero_one_and_many() {
        let none = resolve_simulation_ref_from_summaries(&[])
            .expect_err("empty simulation lists should fail");
        assert!(none.contains("no active simulations"));

        let single = resolve_simulation_ref_from_summaries(&[sample_simulation("sim-1")])
            .expect("single simulation");
        assert_eq!(single, SimulationRef::new("sim-1"));

        let many = resolve_simulation_ref_from_summaries(&[
            sample_simulation("sim-1"),
            sample_simulation("sim-2"),
        ])
        .expect_err("multiple simulations should fail");
        assert!(many.contains("multiple simulations are active"));
    }

    #[test]
    fn parse_helpers_accept_supported_values_and_reject_invalid_input() {
        let apdu = parse_command_apdu("00A4040000").expect("parse apdu");
        assert_eq!(hex::encode_upper(apdu.to_bytes()), "00A4040000");
        assert!(parse_command_apdu("XYZ").is_err());

        let aid = parse_aid("A000000003000000").expect("parse aid");
        assert_eq!(aid.to_hex(), "A000000003000000");
        assert!(parse_aid("invalid").is_err());

        assert_eq!(
            parse_secure_messaging_protocol("iso-7816").expect("iso protocol"),
            SecureMessagingProtocol::Iso7816
        );
        assert_eq!(
            parse_secure_messaging_protocol("scp02").expect("scp02 protocol"),
            SecureMessagingProtocol::Scp02
        );
        assert_eq!(
            parse_secure_messaging_protocol("other:custom").expect("custom protocol"),
            SecureMessagingProtocol::Other("custom".to_string())
        );
        assert!(parse_secure_messaging_protocol("unknown").is_err());
    }

    #[test]
    fn gp_mapping_helpers_convert_cli_enums_to_domain_enums() {
        assert_eq!(
            gp_registry_kind(GpRegistryKindArg::LoadFilesAndModules),
            globalplatform::RegistryKind::ExecutableLoadFilesAndModules
        );
        assert_eq!(
            gp_occurrence(GpOccurrenceArg::Next),
            globalplatform::GetStatusOccurrence::Next
        );
        assert_eq!(
            gp_card_state(GpCardStateArg::CardLocked),
            globalplatform::CardLifeCycle::CardLocked
        );
        assert_eq!(
            gp_transition(GpTransitionArg::Unlock),
            globalplatform::LockTransition::Unlock
        );
    }
}
