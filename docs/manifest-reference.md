# Manifest Reference

JCIM 0.3 uses `jcim.toml` as the project-facing manifest.

## Sections

- `[project]`
  - `name`
  - `profile`
  - `package_name`
  - `package_aid`
  - `applets`
- `[source]`
  - `root`
  - `extra_roots`
- `[build]`
  - `kind = "native" | "command"`
  - `emit = ["cap"]`
  - `command`
  - `cap_output`
  - `dependencies`
  - `version`
- `[simulator]`
  - `auto_build`
  - `reset_after_start`
- `[card]`
  - `default_reader`
  - `default_cap_path`
  - `auto_build_before_install`

## Example

```toml
[project]
name = "demo"
profile = "classic222"
package_name = "com.jcim.demo"
package_aid = "F000000001"

[[project.applets]]
class_name = "com.jcim.demo.DemoApplet"
aid = "F00000000101"

[source]
root = "src/main/javacard"

[build]
kind = "native"
emit = ["cap"]
version = "1.0"

[simulator]
auto_build = true
reset_after_start = false

[card]
auto_build_before_install = true
```
