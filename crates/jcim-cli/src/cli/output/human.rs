use jcim_sdk::iso7816::{FileSelection, IsoSessionState, SecureMessagingProtocol};
use jcim_sdk::{
    BuildSummary, CardAppletInventory, CardDeleteSummary, CardInstallSummary, CardPackageInventory,
    GpSecureChannelSummary, ManageChannelSummary, ProjectDetails, ServiceStatusSummary,
    SimulationStatus, SimulationSummary, globalplatform,
};

pub(super) fn print_project_summary(project: &jcim_sdk::ProjectSummary) {
    println!("Project: {}", project.name);
    println!("Project ID: {}", project.project_id);
    println!("Path: {}", project.project_path.display());
    println!("Profile: {}", project.profile);
    println!("Build: {}", project.build_kind);
    println!(
        "Package: {} ({})",
        project.package_name, project.package_aid
    );
    if !project.applets.is_empty() {
        println!("Applets:");
        for applet in &project.applets {
            println!("  {} ({})", applet.class_name, applet.aid);
        }
    }
}

pub(super) fn print_project_details(details: &ProjectDetails) {
    print_project_summary(&details.project);
    println!();
    println!("{}", details.manifest_toml);
}

pub(super) fn print_build_summary(summary: &BuildSummary, show_rebuilt: bool) {
    print_project_summary(&summary.project);
    if show_rebuilt {
        println!("Rebuilt: {}", if summary.rebuilt { "yes" } else { "no" });
    }
    println!("Artifacts:");
    for artifact in &summary.artifacts {
        println!("  {}: {}", artifact.kind, artifact.path.display());
    }
}

pub(super) fn print_simulation(simulation: &SimulationSummary) {
    println!("Simulation: {}", simulation.simulation_id);
    println!("Project ID: {}", simulation.project_id);
    println!("Project Path: {}", simulation.project_path.display());
    println!("Status: {}", simulation_status_name(simulation.status));
    println!("Reader: {}", simulation.reader_name);
    println!("Health: {}", simulation.health);
    if let Some(atr) = &simulation.atr {
        println!("ATR: {}", atr.to_hex());
    }
    if let Some(protocol) = &simulation.active_protocol
        && let Some(active) = protocol.protocol
    {
        println!("Protocol: {active}");
    }
    println!(
        "Installed package: {} ({})",
        simulation.package_name, simulation.package_aid
    );
    println!(
        "Packages/applets: {}/{}",
        simulation.package_count, simulation.applet_count
    );
    if !simulation.recent_events.is_empty() {
        println!("Events:");
        for event in &simulation.recent_events {
            println!("  {event}");
        }
    }
}

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

pub(super) fn print_card_delete(summary: &CardDeleteSummary) {
    println!("Reader: {}", summary.reader_name);
    println!("Deleted: {}", summary.aid);
    print_output_lines(&summary.output_lines);
}

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

pub(super) fn print_event_lines(lines: &[jcim_sdk::EventLine]) {
    for event in lines {
        println!("[{}] {}", event.level, event.message);
    }
}

pub(super) fn print_plain_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

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

pub(super) fn print_apdu_response(response: &jcim_sdk::ResponseApdu) {
    println!("{}", hex::encode_upper(response.to_bytes()));
}

pub(super) fn print_iso_session_state(state: &IsoSessionState) {
    println!(
        "Power: {}",
        match state.power_state {
            jcim_sdk::iso7816::PowerState::Off => "off",
            jcim_sdk::iso7816::PowerState::On => "on",
        }
    );
    if let Some(atr) = &state.atr {
        println!("ATR: {}", atr.to_hex());
    }
    if let Some(protocol) = &state.active_protocol
        && let Some(active) = protocol.protocol
    {
        println!("Protocol: {active}");
    }
    if let Some(aid) = &state.selected_aid {
        println!("Selected AID: {}", aid.to_hex());
    }
    if let Some(selection) = &state.current_file {
        println!("Current file: {}", file_selection_label(selection));
    }
    if !state.open_channels.is_empty() {
        println!("Channels:");
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
            println!(
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
        println!(
            "Secure messaging: active protocol={} counter={}",
            protocol, state.secure_messaging.command_counter
        );
        if let Some(level) = state.secure_messaging.security_level {
            println!("Security level: {level:02X}");
        }
        if let Some(session_id) = &state.secure_messaging.session_id {
            println!("Session ID: {session_id}");
        }
    } else {
        println!("Secure messaging: inactive");
    }
    if !state.verified_references.is_empty() {
        println!(
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
        println!("Retry counters:");
        for counter in &state.retry_counters {
            println!("  {:02X}: {}", counter.reference, counter.remaining);
        }
    }
    if let Some(status) = state.last_status {
        println!("Last status: {status} ({})", status.label());
    }
}

pub(super) fn print_manage_channel_summary(summary: &ManageChannelSummary) {
    if let Some(channel_number) = summary.channel_number {
        println!("Channel: {channel_number}");
    }
    print_apdu_response(&summary.response);
    println!();
    print_iso_session_state(&summary.session_state);
}

pub(super) fn print_secure_messaging_summary(summary: &jcim_sdk::SecureMessagingSummary) {
    print_iso_session_state(&summary.session_state);
}

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

pub(super) fn print_gp_status_response(response: &globalplatform::GetStatusResponse) {
    println!(
        "Registry: {}",
        match response.kind {
            globalplatform::RegistryKind::IssuerSecurityDomain => "issuer-security-domain",
            globalplatform::RegistryKind::Applications => "applications",
            globalplatform::RegistryKind::ExecutableLoadFiles => "load-files",
            globalplatform::RegistryKind::ExecutableLoadFilesAndModules => "load-files-and-modules",
        }
    );
    println!(
        "More data available: {}",
        if response.more_data_available {
            "yes"
        } else {
            "no"
        }
    );
    if response.entries.is_empty() {
        println!("Entries: none");
        return;
    }
    println!("Entries:");
    for entry in &response.entries {
        println!("  AID: {}", entry.aid.to_hex());
        println!("  Life cycle: {:02X}", entry.life_cycle_state);
        if let Some(privileges) = entry.privileges {
            println!(
                "  Privileges: {}",
                hex::encode_upper([privileges[0], privileges[1], privileges[2]])
            );
        }
        if let Some(aid) = &entry.executable_load_file_aid {
            println!("  Load file: {}", aid.to_hex());
        }
        if let Some(aid) = &entry.associated_security_domain_aid {
            println!("  Associated SD: {}", aid.to_hex());
        }
        if !entry.executable_module_aids.is_empty() {
            println!(
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
            println!("  Version: {}", hex::encode_upper(version));
        }
        if !entry.implicit_selection_parameters.is_empty() {
            println!(
                "  Implicit selection: {}",
                hex::encode_upper(&entry.implicit_selection_parameters)
            );
        }
        println!();
    }
}

pub(super) fn print_service_status(response: &ServiceStatusSummary) {
    println!("Socket: {}", response.socket_path.display());
    println!("Running: {}", if response.running { "yes" } else { "no" });
    println!("Known projects: {}", response.known_project_count);
    println!("Active simulations: {}", response.active_simulation_count);
}

fn print_output_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

fn file_selection_label(selection: &FileSelection) -> String {
    match selection {
        FileSelection::ByName(bytes) => format!("name:{}", hex::encode_upper(bytes)),
        FileSelection::FileId(file_id) => format!("fid:{file_id:04X}"),
        FileSelection::Path(path) => format!("path:{}", hex::encode_upper(path)),
    }
}

fn simulation_status_name(status: SimulationStatus) -> &'static str {
    match status {
        SimulationStatus::Starting => "starting",
        SimulationStatus::Running => "running",
        SimulationStatus::Stopped => "stopped",
        SimulationStatus::Failed => "failed",
        SimulationStatus::Unknown => "unknown",
    }
}
