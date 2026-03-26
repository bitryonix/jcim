//! Bundled Java Card toolchain discovery and profile selection.

use std::path::PathBuf;

use jcim_core::error::{JcimError, Result};
use jcim_core::model::CardProfileId;

use super::types::ToolchainLayout;

/// Resolve the bundled Java build toolchain shipped under `third_party/`.
pub fn build_toolchain_layout() -> Result<ToolchainLayout> {
    let third_party_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("third_party")
        .canonicalize()
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("third_party")
        });
    let layout = ToolchainLayout {
        ecj_jar: third_party_root.join("ecj/ecj.jar"),
        ant_javacard_jar: third_party_root.join("ant_javacard/ant-javacard.jar"),
        sdk_root: third_party_root.join("javacard_sdks"),
        third_party_root,
    };
    for (label, path) in [
        ("ecj.jar", &layout.ecj_jar),
        ("ant-javacard.jar", &layout.ant_javacard_jar),
        ("Java Card SDK root", &layout.sdk_root),
    ] {
        if !path.exists() {
            return Err(JcimError::Unsupported(format!(
                "bundled build toolchain is incomplete: missing {label} at {}",
                path.display()
            )));
        }
    }
    Ok(layout)
}

/// Profile-specific Java Card SDK inputs and converter settings derived from the bundled toolchain.
#[derive(Clone, Debug)]
pub(crate) struct ProfileToolchain {
    /// API jar exposed to the Java compiler.
    pub(crate) api_jar: PathBuf,
    /// Export-file directory consumed by the CAP converter.
    pub(crate) export_dir: PathBuf,
    /// Converter-side jars required on the Java classpath.
    pub(crate) tool_jars: Vec<PathBuf>,
    /// Main class used to launch the selected converter version.
    pub(crate) converter_class: &'static str,
    /// ECJ compliance level compatible with the target profile's classfile expectations.
    pub(crate) ecj_compliance: &'static str,
    /// Converter output kinds requested for the target profile.
    pub(crate) converter_outputs: &'static [&'static str],
    /// Whether the converter expects the `-useproxyclass` flag.
    pub(crate) use_proxy_class: bool,
    /// Whether converter verification should be disabled for the newer classic profiles.
    pub(crate) no_verify: bool,
}

/// Resolve the bundled SDK jars, export files, and converter settings for one JCIM profile.
pub(crate) fn profile_toolchain(
    profile: CardProfileId,
    toolchain: &ToolchainLayout,
) -> Result<ProfileToolchain> {
    let root = &toolchain.sdk_root;
    let (kit_dir, api_jar, tool_jars, converter_class, ecj_compliance, outputs, use_proxy_class) =
        match profile {
            CardProfileId::Classic211 => (
                "jc211_kit",
                "bin/api.jar",
                vec!["bin/converter.jar", "bin/api.jar"],
                "com.sun.javacard.converter.Converter",
                "-1.3",
                &["CAP", "EXP"][..],
                false,
            ),
            CardProfileId::Classic221 => (
                "jc221_kit",
                "lib/api.jar",
                vec![
                    "lib/converter.jar",
                    "lib/offcardverifier.jar",
                    "lib/api.jar",
                ],
                "com.sun.javacard.converter.Converter",
                "-1.4",
                &["CAP", "EXP"][..],
                false,
            ),
            CardProfileId::Classic222 => (
                "jc222_kit",
                "lib/api.jar",
                vec![
                    "lib/converter.jar",
                    "lib/offcardverifier.jar",
                    "lib/api.jar",
                ],
                "com.sun.javacard.converter.Converter",
                "-1.5",
                &["CAP", "EXP"][..],
                false,
            ),
            CardProfileId::Classic301 => (
                "jc303_kit",
                "lib/api_classic.jar",
                vec!["lib/tools.jar"],
                "com.sun.javacard.converter.Main",
                "-1.6",
                &["CAP", "EXP", "JCA"][..],
                true,
            ),
            CardProfileId::Classic304 => (
                "jc304_kit",
                "lib/api_classic.jar",
                vec!["lib/tools.jar"],
                "com.sun.javacard.converter.Main",
                "-1.6",
                &["CAP", "EXP", "JCA"][..],
                true,
            ),
            CardProfileId::Classic305 => (
                "jc305u4_kit",
                "lib/api_classic.jar",
                vec!["lib/tools.jar"],
                "com.sun.javacard.converter.Main",
                "-1.6",
                &["CAP", "EXP", "JCA"][..],
                true,
            ),
            other => {
                return Err(JcimError::Unsupported(format!(
                    "source-first CAP building is not wired for profile {other}"
                )));
            }
        };
    let tool_jars = tool_jars
        .into_iter()
        .map(|entry| root.join(kit_dir).join(entry))
        .collect::<Vec<_>>();
    let resolved = ProfileToolchain {
        api_jar: root.join(kit_dir).join(api_jar),
        export_dir: root.join(kit_dir).join("api_export_files"),
        tool_jars,
        converter_class,
        ecj_compliance,
        converter_outputs: outputs,
        use_proxy_class,
        no_verify: matches!(
            profile,
            CardProfileId::Classic301 | CardProfileId::Classic304 | CardProfileId::Classic305
        ),
    };
    for (label, path) in std::iter::once(("API jar", &resolved.api_jar))
        .chain(std::iter::once(("export dir", &resolved.export_dir)))
        .chain(resolved.tool_jars.iter().map(|path| ("tool jar", path)))
    {
        if !path.exists() {
            return Err(JcimError::Unsupported(format!(
                "bundled Java Card toolchain is missing {label} at {}",
                path.display()
            )));
        }
    }
    Ok(resolved)
}
