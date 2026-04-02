use std::fmt::Write as _;

use jcim_sdk::iso7816::{FileSelection, IsoSessionState, SecureMessagingProtocol};
use jcim_sdk::{
    BuildSummary, CardAppletInventory, CardDeleteSummary, CardInstallSummary, CardPackageInventory,
    GpSecureChannelSummary, ManageChannelSummary, ProjectDetails, ServiceStatusSummary,
    SimulationStatus, SimulationSummary, globalplatform,
};

/// Render one project summary into the stable human-readable CLI block.
pub(super) fn render_project_summary(project: &jcim_sdk::ProjectSummary) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Project: {}", project.name);
    let _ = writeln!(output, "Project ID: {}", project.project_id);
    let _ = writeln!(output, "Path: {}", project.project_path.display());
    let _ = writeln!(output, "Profile: {}", project.profile);
    let _ = writeln!(output, "Build: {}", project.build_kind);
    let _ = writeln!(
        output,
        "Package: {} ({})",
        project.package_name, project.package_aid
    );
    if !project.applets.is_empty() {
        let _ = writeln!(output, "Applets:");
        for applet in &project.applets {
            let _ = writeln!(output, "  {} ({})", applet.class_name, applet.aid);
        }
    }
    output
}

/// Print one project summary using the human-readable renderer.
pub(super) fn print_project_summary(project: &jcim_sdk::ProjectSummary) {
    print!("{}", render_project_summary(project));
}

/// Print one project-details payload in the human-readable CLI format.
pub(super) fn print_project_details(details: &ProjectDetails) {
    print!("{}", render_project_summary(&details.project));
    println!();
    println!("{}", details.manifest_toml);
}

/// Print one build summary in the human-readable CLI format.
pub(super) fn print_build_summary(summary: &BuildSummary, show_rebuilt: bool) {
    print!("{}", render_project_summary(&summary.project));
    if show_rebuilt {
        println!("Rebuilt: {}", if summary.rebuilt { "yes" } else { "no" });
    }
    println!("Artifacts:");
    for artifact in &summary.artifacts {
        println!("  {}: {}", artifact.kind, artifact.path.display());
    }
}

/// Render one simulation summary into the stable human-readable CLI block.
pub(super) fn render_simulation(simulation: &SimulationSummary) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Simulation: {}", simulation.simulation_id);
    let _ = writeln!(output, "Project ID: {}", simulation.project_id);
    let _ = writeln!(
        output,
        "Project Path: {}",
        simulation.project_path.display()
    );
    let _ = writeln!(
        output,
        "Status: {}",
        simulation_status_name(simulation.status)
    );
    let _ = writeln!(output, "Reader: {}", simulation.reader_name);
    let _ = writeln!(output, "Health: {}", simulation.health);
    if let Some(atr) = &simulation.atr {
        let _ = writeln!(output, "ATR: {}", atr.to_hex());
    }
    if let Some(protocol) = &simulation.active_protocol
        && let Some(active) = protocol.protocol
    {
        let _ = writeln!(output, "Protocol: {active}");
    }
    let _ = writeln!(
        output,
        "Installed package: {} ({})",
        simulation.package_name, simulation.package_aid
    );
    let _ = writeln!(
        output,
        "Packages/applets: {}/{}",
        simulation.package_count, simulation.applet_count
    );
    if !simulation.recent_events.is_empty() {
        let _ = writeln!(output, "Events:");
        for event in &simulation.recent_events {
            let _ = writeln!(output, "  {event}");
        }
    }
    output
}

/// Print one simulation summary using the human-readable renderer.
pub(super) fn print_simulation(simulation: &SimulationSummary) {
    print!("{}", render_simulation(simulation));
}

/// Print one card-install summary in the human-readable CLI format.
pub(super) fn print_card_install(summary: &CardInstallSummary) {
    println!("Reader: {}", summary.reader_name);
    println!("CAP: {}", summary.cap_path.display());
    println!(
        "Installed package: {} ({})",
        summary.package_name, summary.package_aid
    );
    if !summary.applets.is_empty() {
        println!("Applets:");
        for applet in &summary.applets {
            println!("  {} ({})", applet.class_name, applet.aid);
        }
    }
    print_output_lines(&summary.output_lines);
}

/// Print one card-delete summary in the human-readable CLI format.
pub(super) fn print_card_delete(summary: &CardDeleteSummary) {
    println!("Reader: {}", summary.reader_name);
    println!("Deleted: {}", summary.aid);
    print_output_lines(&summary.output_lines);
}

/// Print one package inventory in the human-readable CLI format.
pub(super) fn print_package_inventory(inventory: &CardPackageInventory) {
    if inventory.packages.is_empty() {
        print_output_lines(&inventory.output_lines);
    } else {
        println!("Reader: {}", inventory.reader_name);
        for package in &inventory.packages {
            if package.description.is_empty() {
                println!("{}", package.aid);
            } else {
                println!("{} {}", package.aid, package.description);
            }
        }
    }
}

/// Print one applet inventory in the human-readable CLI format.
pub(super) fn print_applet_inventory(inventory: &CardAppletInventory) {
    if inventory.applets.is_empty() {
        print_output_lines(&inventory.output_lines);
    } else {
        println!("Reader: {}", inventory.reader_name);
        for applet in &inventory.applets {
            if applet.description.is_empty() {
                println!("{}", applet.aid);
            } else {
                println!("{} {}", applet.aid, applet.description);
            }
        }
    }
}

/// Print event lines exactly as stored by the service state.
pub(super) fn print_event_lines(lines: &[jcim_sdk::EventLine]) {
    for event in lines {
        println!("[{}] {}", event.level, event.message);
    }
}

/// Print plain line-oriented output returned by service helpers.
pub(super) fn print_plain_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

/// Print one reset summary in the human-readable CLI format.
pub(super) fn print_reset_summary(summary: &jcim_sdk::ResetSummary) {
    println!(
        "{}",
        summary
            .atr
            .as_ref()
            .map(|atr| hex::encode_upper(&atr.raw))
            .unwrap_or_default()
    );
}

/// Print the discovered reader list in the human-readable CLI format.
pub(super) fn print_card_readers(readers: &[jcim_sdk::CardReaderSummary]) {
    if readers.is_empty() {
        println!("No PC/SC readers found.");
    } else {
        for reader in readers {
            println!(
                "{}\t{}",
                reader.name,
                if reader.card_present {
                    "present"
                } else {
                    "empty"
                }
            );
        }
    }
}

/// Print one APDU response in the compact human-readable CLI format.
pub(super) fn print_apdu_response(response: &jcim_sdk::ResponseApdu) {
    println!("{}", hex::encode_upper(response.to_bytes()));
}

/// Render one ISO session state into the stable human-readable CLI block.
pub(super) fn render_iso_session_state(state: &IsoSessionState) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "Power: {}",
        match state.power_state {
            jcim_sdk::iso7816::PowerState::Off => "off",
            jcim_sdk::iso7816::PowerState::On => "on",
        }
    );
    if let Some(atr) = &state.atr {
        let _ = writeln!(output, "ATR: {}", atr.to_hex());
    }
    if let Some(protocol) = &state.active_protocol
        && let Some(active) = protocol.protocol
    {
        let _ = writeln!(output, "Protocol: {active}");
    }
    if let Some(aid) = &state.selected_aid {
        let _ = writeln!(output, "Selected AID: {}", aid.to_hex());
    }
    if let Some(selection) = &state.current_file {
        let _ = writeln!(output, "Current file: {}", file_selection_label(selection));
    }
    if !state.open_channels.is_empty() {
        let _ = writeln!(output, "Channels:");
        for channel in &state.open_channels {
            let selected = channel
                .selected_aid
                .as_ref()
                .map(|aid| aid.to_hex())
                .unwrap_or_else(|| "-".to_string());
            let current_file = channel
                .current_file
                .as_ref()
                .map(file_selection_label)
                .unwrap_or_else(|| "-".to_string());
            let _ = writeln!(
                output,
                "  {}  selected={} file={}",
                channel.channel_number, selected, current_file
            );
        }
    }
    if state.secure_messaging.active {
        let protocol = match &state.secure_messaging.protocol {
            Some(SecureMessagingProtocol::Iso7816) => "iso7816".to_string(),
            Some(SecureMessagingProtocol::Scp02) => "scp02".to_string(),
            Some(SecureMessagingProtocol::Scp03) => "scp03".to_string(),
            Some(SecureMessagingProtocol::Other(label)) => format!("other:{label}"),
            None => "unknown".to_string(),
        };
        let _ = writeln!(
            output,
            "Secure messaging: active protocol={} counter={}",
            protocol, state.secure_messaging.command_counter
        );
        if let Some(level) = state.secure_messaging.security_level {
            let _ = writeln!(output, "Security level: {level:02X}");
        }
        if let Some(session_id) = &state.secure_messaging.session_id {
            let _ = writeln!(output, "Session ID: {session_id}");
        }
    } else {
        let _ = writeln!(output, "Secure messaging: inactive");
    }
    if !state.verified_references.is_empty() {
        let _ = writeln!(
            output,
            "Verified references: {}",
            state
                .verified_references
                .iter()
                .map(|value| format!("{value:02X}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !state.retry_counters.is_empty() {
        let _ = writeln!(output, "Retry counters:");
        for counter in &state.retry_counters {
            let _ = writeln!(output, "  {:02X}: {}", counter.reference, counter.remaining);
        }
    }
    if let Some(status) = state.last_status {
        let _ = writeln!(output, "Last status: {status} ({})", status.label());
    }
    output
}

/// Print one ISO session state using the human-readable renderer.
pub(super) fn print_iso_session_state(state: &IsoSessionState) {
    print!("{}", render_iso_session_state(state));
}

/// Print one manage-channel summary in the human-readable CLI format.
pub(super) fn print_manage_channel_summary(summary: &ManageChannelSummary) {
    if let Some(channel_number) = summary.channel_number {
        println!("Channel: {channel_number}");
    }
    print_apdu_response(&summary.response);
    println!();
    print_iso_session_state(&summary.session_state);
}

/// Print one secure-messaging summary in the human-readable CLI format.
pub(super) fn print_secure_messaging_summary(summary: &jcim_sdk::SecureMessagingSummary) {
    print_iso_session_state(&summary.session_state);
}

/// Print one GP secure-channel summary in the human-readable CLI format.
pub(super) fn print_gp_secure_channel_summary(summary: &GpSecureChannelSummary) {
    println!("Keyset: {}", summary.secure_channel.keyset.name);
    println!(
        "Protocol: {}",
        match summary.secure_channel.keyset.mode {
            globalplatform::ScpMode::Scp02 => "scp02",
            globalplatform::ScpMode::Scp03 => "scp03",
        }
    );
    println!(
        "Security level: {:02X}",
        summary.secure_channel.security_level.as_byte()
    );
    println!("Session ID: {}", summary.secure_channel.session_id);
    println!("Selected AID: {}", summary.selected_aid.to_hex());
    println!();
    print_iso_session_state(&summary.session_state);
}

/// Render one GP registry-status response into the stable human-readable CLI block.
pub(super) fn render_gp_status_response(response: &globalplatform::GetStatusResponse) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "Registry: {}",
        match response.kind {
            globalplatform::RegistryKind::IssuerSecurityDomain => "issuer-security-domain",
            globalplatform::RegistryKind::Applications => "applications",
            globalplatform::RegistryKind::ExecutableLoadFiles => "load-files",
            globalplatform::RegistryKind::ExecutableLoadFilesAndModules => "load-files-and-modules",
        }
    );
    let _ = writeln!(
        output,
        "More data available: {}",
        if response.more_data_available {
            "yes"
        } else {
            "no"
        }
    );
    if response.entries.is_empty() {
        let _ = writeln!(output, "Entries: none");
        return output;
    }
    let _ = writeln!(output, "Entries:");
    for entry in &response.entries {
        let _ = writeln!(output, "  AID: {}", entry.aid.to_hex());
        let _ = writeln!(output, "  Life cycle: {:02X}", entry.life_cycle_state);
        if let Some(privileges) = entry.privileges {
            let _ = writeln!(
                output,
                "  Privileges: {}",
                hex::encode_upper([privileges[0], privileges[1], privileges[2]])
            );
        }
        if let Some(aid) = &entry.executable_load_file_aid {
            let _ = writeln!(output, "  Load file: {}", aid.to_hex());
        }
        if let Some(aid) = &entry.associated_security_domain_aid {
            let _ = writeln!(output, "  Associated SD: {}", aid.to_hex());
        }
        if !entry.executable_module_aids.is_empty() {
            let _ = writeln!(
                output,
                "  Modules: {}",
                entry
                    .executable_module_aids
                    .iter()
                    .map(|aid| aid.to_hex())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if let Some(version) = &entry.load_file_version {
            let _ = writeln!(output, "  Version: {}", hex::encode_upper(version));
        }
        if !entry.implicit_selection_parameters.is_empty() {
            let _ = writeln!(
                output,
                "  Implicit selection: {}",
                hex::encode_upper(&entry.implicit_selection_parameters)
            );
        }
        output.push('\n');
    }
    output
}

/// Print one GP registry-status response using the human-readable renderer.
pub(super) fn print_gp_status_response(response: &globalplatform::GetStatusResponse) {
    print!("{}", render_gp_status_response(response));
}

/// Render one service-status summary into the stable human-readable CLI block.
pub(super) fn render_service_status(response: &ServiceStatusSummary) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Socket: {}", response.socket_path.display());
    let _ = writeln!(
        output,
        "Running: {}",
        if response.running { "yes" } else { "no" }
    );
    let _ = writeln!(output, "Known projects: {}", response.known_project_count);
    let _ = writeln!(
        output,
        "Active simulations: {}",
        response.active_simulation_count
    );
    output
}

/// Print one service-status summary using the human-readable renderer.
pub(super) fn print_service_status(response: &ServiceStatusSummary) {
    print!("{}", render_service_status(response));
}

/// Print a pre-rendered line list with one trailing newline per line.
fn print_output_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

/// Render one file-selection variant for human-readable CLI output.
fn file_selection_label(selection: &FileSelection) -> String {
    match selection {
        FileSelection::ByName(bytes) => format!("name:{}", hex::encode_upper(bytes)),
        FileSelection::FileId(file_id) => format!("fid:{file_id:04X}"),
        FileSelection::Path(path) => format!("path:{}", hex::encode_upper(path)),
    }
}

/// Return the stable human-readable name for one simulation lifecycle status.
fn simulation_status_name(status: SimulationStatus) -> &'static str {
    match status {
        SimulationStatus::Starting => "starting",
        SimulationStatus::Running => "running",
        SimulationStatus::Stopped => "stopped",
        SimulationStatus::Failed => "failed",
        SimulationStatus::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jcim_core::aid::Aid;
    use jcim_core::globalplatform::{GetStatusResponse, RegistryEntry, RegistryKind};
    use jcim_sdk::iso7816::{
        Atr, IsoCapabilities, IsoSessionState, LogicalChannelState, PowerState, ProtocolParameters,
        RetryCounterState, SecureMessagingProtocol, SecureMessagingState, StatusWord,
        TransportProtocol,
    };
    use jcim_sdk::{AppletSummary, ProjectSummary, ServiceStatusSummary};

    use super::{
        render_gp_status_response, render_iso_session_state, render_project_summary,
        render_service_status,
    };

    #[test]
    fn render_project_summary_is_stable_and_human_readable() {
        let rendered = render_project_summary(&ProjectSummary {
            project_id: "proj-1".to_string(),
            name: "Demo".to_string(),
            project_path: PathBuf::from("/tmp/demo"),
            profile: "classic305".to_string(),
            build_kind: "native".to_string(),
            package_name: "com.example.demo".to_string(),
            package_aid: "A000000151000001".to_string(),
            applets: vec![AppletSummary {
                class_name: "com.example.demo.DemoApplet".to_string(),
                aid: "A000000151000002".to_string(),
            }],
        });

        assert!(rendered.contains("Project: Demo"));
        assert!(rendered.contains("Package: com.example.demo (A000000151000001)"));
        assert!(rendered.contains("Applets:"));
    }

    #[test]
    fn render_iso_session_state_includes_secure_messaging_and_retry_details() {
        let atr = Atr::parse(&[0x3B, 0x80, 0x01, 0x00]).expect("atr");
        let rendered = render_iso_session_state(&IsoSessionState {
            power_state: PowerState::On,
            atr: Some(atr.clone()),
            active_protocol: Some(ProtocolParameters::from_atr(&atr)),
            selected_aid: Some(Aid::from_hex("A000000151000001").expect("aid")),
            current_file: None,
            open_channels: vec![LogicalChannelState {
                channel_number: 0,
                selected_aid: Some(Aid::from_hex("A000000151000001").expect("aid")),
                current_file: None,
            }],
            secure_messaging: SecureMessagingState {
                active: true,
                protocol: Some(SecureMessagingProtocol::Scp03),
                security_level: Some(0x13),
                session_id: Some("session-1".to_string()),
                command_counter: 9,
            },
            verified_references: vec![0x81, 0x82],
            retry_counters: vec![RetryCounterState {
                reference: 0x81,
                remaining: 3,
            }],
            last_status: Some(StatusWord::new(0x63C2)),
        });

        assert!(rendered.contains("Power: on"));
        assert!(rendered.contains("Secure messaging: active protocol=scp03 counter=9"));
        assert!(rendered.contains("Verified references: 81, 82"));
        assert!(rendered.contains("Retry counters:"));
        assert!(rendered.contains("Last status: 63C2"));
    }

    #[test]
    fn render_gp_status_response_and_service_status_are_stable() {
        let gp_rendered = render_gp_status_response(&GetStatusResponse {
            kind: RegistryKind::Applications,
            entries: vec![RegistryEntry {
                kind: RegistryKind::Applications,
                aid: Aid::from_hex("A000000151000001").expect("aid"),
                life_cycle_state: 0x07,
                privileges: Some([0x01, 0x02, 0x03]),
                executable_load_file_aid: None,
                associated_security_domain_aid: None,
                executable_module_aids: Vec::new(),
                load_file_version: Some(vec![1, 2]),
                implicit_selection_parameters: vec![0xAA],
            }],
            more_data_available: true,
        });
        assert!(gp_rendered.contains("Registry: applications"));
        assert!(gp_rendered.contains("More data available: yes"));
        assert!(gp_rendered.contains("Privileges: 010203"));

        let service_rendered = render_service_status(&ServiceStatusSummary {
            socket_path: PathBuf::from("/tmp/jcim.sock"),
            running: true,
            known_project_count: 3,
            active_simulation_count: 2,
            service_binary_path: PathBuf::from("/tmp/jcimd"),
            service_binary_fingerprint: "fp".to_string(),
        });
        assert!(service_rendered.contains("Socket: /tmp/jcim.sock"));
        assert!(service_rendered.contains("Running: yes"));
        assert!(service_rendered.contains("Known projects: 3"));
        assert!(service_rendered.contains("Active simulations: 2"));
    }

    #[test]
    fn render_iso_session_state_reports_inactive_secure_messaging_when_absent() {
        let rendered = render_iso_session_state(&IsoSessionState {
            power_state: PowerState::Off,
            atr: None,
            active_protocol: None,
            selected_aid: None,
            current_file: None,
            open_channels: Vec::new(),
            secure_messaging: SecureMessagingState::default(),
            verified_references: Vec::new(),
            retry_counters: Vec::new(),
            last_status: None,
        });

        assert!(rendered.contains("Power: off"));
        assert!(rendered.contains("Secure messaging: inactive"));
    }

    #[allow(dead_code)]
    fn _keep_iso_capabilities_type_visible(_: IsoCapabilities, _: TransportProtocol) {}
}
