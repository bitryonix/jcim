//! Native and external build execution helpers.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use jcim_config::project::ArtifactKind;
use jcim_core::aid::Aid;
use jcim_core::error::{JcimError, Result};

use super::fingerprint::discover_java_sources;
use super::metadata::{project_build_root, relativize_path, write_simulator_metadata};
use super::toolchain::{ProfileToolchain, profile_toolchain};
use super::types::{ArtifactMetadata, BuildArtifactRequest, ToolchainLayout};

/// Build a JCIM-native source tree into CAP artifacts under `.jcim/build/`.
pub(crate) fn build_jcim_project(
    request: &BuildArtifactRequest,
    toolchain: &ToolchainLayout,
    fingerprint: &str,
) -> Result<ArtifactMetadata> {
    let profile_tool = profile_toolchain(request.profile, toolchain)?;
    let build_root = project_build_root(&request.project_root);
    let classes_dir = build_root.join("classes");
    let converter_dir = build_root.join("converter");
    let cap_dir = build_root.join("cap");
    reset_directory(&classes_dir)?;
    reset_directory(&converter_dir)?;
    std::fs::create_dir_all(&cap_dir)?;

    let java_files = discover_java_sources(request)?;
    if java_files.is_empty() {
        return Err(JcimError::Unsupported(
            "no Java source files found under any configured source root".to_string(),
        ));
    }

    let compile_classpath = classpath_string(
        std::iter::once(profile_tool.api_jar.clone())
            .chain(request.dependencies.iter().cloned())
            .collect::<Vec<_>>(),
    );
    run_command(
        Command::new("java")
            .arg("-jar")
            .arg(&toolchain.ecj_jar)
            .arg(profile_tool.ecj_compliance)
            .arg("-proc:none")
            .arg("-classpath")
            .arg(compile_classpath)
            .arg("-d")
            .arg(&classes_dir)
            .args(java_files.iter().map(|path| path.as_os_str())),
        "compile Java Card sources with ecj",
    )?;

    let simulator_metadata_path = build_root.join("simulator.properties");
    write_simulator_metadata(
        &simulator_metadata_path,
        &request.package_name,
        &request.package_aid,
        &request.version,
        &request.applets,
    )?;

    let cap_path = if request.emit.contains(&ArtifactKind::Cap) {
        run_converter(request, &profile_tool, &classes_dir, &converter_dir)?;
        let generated = converter_dir
            .join(request.package_name.replace('.', "/"))
            .join("javacard")
            .join(format!(
                "{}.cap",
                request
                    .package_name
                    .split('.')
                    .next_back()
                    .unwrap_or("applet")
            ));
        if !generated.exists() {
            return Err(JcimError::Unsupported(format!(
                "converter completed without producing a CAP archive at {}",
                generated.display()
            )));
        }
        let output = cap_dir.join(format!("{}.cap", package_file_stem(&request.package_name)));
        std::fs::copy(&generated, &output)?;
        Some(output)
    } else {
        None
    };

    Ok(ArtifactMetadata {
        build_kind: request.build_kind,
        profile: request.profile,
        package_name: request.package_name.clone(),
        package_aid: request.package_aid.clone(),
        version: request.version.clone(),
        applets: request.applets.clone(),
        cap_path: cap_path
            .as_ref()
            .map(|path| relativize_path(&request.project_root, path)),
        classes_path: relativize_path(&request.project_root, &classes_dir),
        simulator_metadata_path: relativize_path(&request.project_root, &simulator_metadata_path),
        source_fingerprint: fingerprint.to_string(),
    })
}

/// Run a caller-supplied external build command and harvest the declared artifact outputs.
pub(crate) fn build_external_project(
    request: &BuildArtifactRequest,
    fingerprint: &str,
) -> Result<ArtifactMetadata> {
    let build_cmd = request.command.as_ref().ok_or_else(|| {
        JcimError::Unsupported(format!(
            "build.kind = {:?} requires [build].command in jcim.toml",
            request.build_kind
        ))
    })?;
    run_command(
        Command::new("/bin/sh")
            .arg("-lc")
            .arg(build_cmd)
            .current_dir(&request.project_root),
        "run external Java Card build command",
    )?;

    let build_root = project_build_root(&request.project_root);
    std::fs::create_dir_all(&build_root)?;
    let simulator_metadata_path = build_root.join("simulator.properties");
    write_simulator_metadata(
        &simulator_metadata_path,
        &request.package_name,
        &request.package_aid,
        &request.version,
        &request.applets,
    )?;
    let classes_dir = build_root.join("classes");
    if !classes_dir.exists() {
        std::fs::create_dir_all(&classes_dir)?;
    }

    let cap_path = request
        .cap_output
        .as_ref()
        .and_then(|path| path.exists().then_some(path));
    if request.emit.contains(&ArtifactKind::Cap) && cap_path.is_none() {
        return Err(JcimError::Unsupported(
            "external build finished without the declared CAP output".to_string(),
        ));
    }

    Ok(ArtifactMetadata {
        build_kind: request.build_kind,
        profile: request.profile,
        package_name: request.package_name.clone(),
        package_aid: request.package_aid.clone(),
        version: request.version.clone(),
        applets: request.applets.clone(),
        cap_path: cap_path.map(|path| relativize_path(&request.project_root, path)),
        classes_path: relativize_path(&request.project_root, &classes_dir),
        simulator_metadata_path: relativize_path(&request.project_root, &simulator_metadata_path),
        source_fingerprint: fingerprint.to_string(),
    })
}

/// Invoke the bundled Java Card converter for one compiled source tree.
fn run_converter(
    request: &BuildArtifactRequest,
    profile_tool: &ProfileToolchain,
    classes_dir: &Path,
    converter_dir: &Path,
) -> Result<()> {
    let export_path = classpath_string(
        std::iter::once(profile_tool.export_dir.clone())
            .chain(request.dependencies.iter().cloned())
            .collect::<Vec<_>>(),
    );
    let mut command = Command::new("java");
    command
        .arg("-cp")
        .arg(classpath_string(profile_tool.tool_jars.clone()))
        .arg(profile_tool.converter_class)
        .arg("-d")
        .arg(converter_dir)
        .arg("-classdir")
        .arg(classes_dir)
        .arg("-exportpath")
        .arg(export_path)
        .arg("-verbose")
        .arg("-nobanner")
        .arg("-out");

    for value in profile_tool.converter_outputs {
        command.arg(value);
    }
    if profile_tool.use_proxy_class {
        command.arg("-useproxyclass");
    }
    if profile_tool.no_verify {
        command.arg("-noverify");
    }
    for applet in &request.applets {
        command
            .arg("-applet")
            .arg(format_aid_for_converter(&applet.aid))
            .arg(&applet.class_name);
    }
    command
        .arg(&request.package_name)
        .arg(format_aid_for_converter(&request.package_aid))
        .arg(&request.version);

    run_command(&mut command, "convert compiled classes into a CAP archive")
}

/// Recreate one directory from scratch before a fresh build step writes into it.
fn reset_directory(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    std::fs::create_dir_all(path)?;
    Ok(())
}

/// Join path entries using the platform classpath separator.
fn classpath_string(paths: Vec<PathBuf>) -> String {
    let separator = if cfg!(windows) { ";" } else { ":" };
    paths
        .into_iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(separator)
}

/// Format one AID in the converter CLI syntax expected by Oracle tooling.
pub(crate) fn format_aid_for_converter(aid: &Aid) -> String {
    aid.as_bytes()
        .iter()
        .map(|byte| format!("0x{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Normalize a Java package name into a filesystem-friendly artifact stem.
fn package_file_stem(package_name: &str) -> String {
    package_name.replace('.', "_")
}

/// Run one external tool command and attach actionable context on failure.
fn run_command(command: &mut Command, description: &str) -> Result<()> {
    let rendered = format!("{command:?}");
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| {
            JcimError::Unsupported(format!(
                "unable to {description} with `{rendered}`: {error}"
            ))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(JcimError::Unsupported(format!(
            "failed to {description} with `{rendered}`; exit status {status}"
        )))
    }
}
