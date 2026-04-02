use super::*;

/// Parse package inventory lines emitted by helper-tool and normalize AIDs.
pub(super) fn parse_package_inventory(lines: &[String]) -> Vec<CardPackageSummary> {
    lines
        .iter()
        .filter_map(|line| parse_inventory_item(line, "PKG: "))
        .map(|(aid, description)| CardPackageSummary { aid, description })
        .collect()
}

/// Parse applet inventory lines emitted by helper-tool and normalize AIDs.
pub(super) fn parse_applet_inventory(lines: &[String]) -> Vec<CardAppletSummary> {
    lines
        .iter()
        .filter_map(|line| parse_inventory_item(line, "APP: "))
        .map(|(aid, description)| CardAppletSummary { aid, description })
        .collect()
}

/// Parse one prefixed inventory line into a normalized AID plus human-readable description.
fn parse_inventory_item(line: &str, prefix: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix(prefix)?.trim();
    let mut parts = rest.split_whitespace();
    let aid = parts.next()?;
    let parsed = Aid::from_hex(aid).ok()?;
    let description = parts.collect::<Vec<_>>().join(" ");
    Some((parsed.to_hex(), description))
}

#[cfg(test)]
mod tests {
    use super::{parse_applet_inventory, parse_package_inventory};

    #[test]
    fn parses_package_inventory_lines() {
        let packages = parse_package_inventory(&[
            "PKG: A000000151000000 demo.package 1.0".to_string(),
            "ISD: A000000003000000".to_string(),
        ]);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].aid, "A000000151000000");
        assert_eq!(packages[0].description, "demo.package 1.0");
    }

    #[test]
    fn parses_applet_inventory_lines() {
        let applets = parse_applet_inventory(&[
            "APP: A000000151000001 DemoApplet".to_string(),
            "APP: invalid broken".to_string(),
        ]);
        assert_eq!(applets.len(), 1);
        assert_eq!(applets[0].aid, "A000000151000001");
        assert_eq!(applets[0].description, "DemoApplet");
    }
}
