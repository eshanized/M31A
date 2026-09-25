use crate::repo::types::{FactClass, RepositorySymbol, SymbolKind};
use serde::Deserialize;
use std::collections::BTreeMap;

pub struct ManifestExtractor;

#[derive(Debug, Deserialize)]
struct CargoToml {
    package: Option<CargoPackage>,
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: BTreeMap<String, toml::Value>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: BTreeMap<String, toml::Value>,
    #[serde(default)]
    bin: Vec<CargoTarget>,
    lib: Option<CargoTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
    version: Option<String>,
    edition: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    name: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PackageJson {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    #[serde(default)]
    scripts: BTreeMap<String, String>,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct PyProjectToml {
    project: Option<PyProjectDetails>,
    tool: Option<PyProjectTool>,
}

#[derive(Debug, Deserialize)]
struct PyProjectDetails {
    name: String,
    version: Option<String>,
    description: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    scripts: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct PyProjectTool {
    poetry: Option<PoetryDetails>,
}

#[derive(Debug, Deserialize)]
struct PoetryDetails {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,
    #[serde(default)]
    scripts: BTreeMap<String, String>,
}

impl ManifestExtractor {
    pub fn extract_cargo(file_path: &str, content: &str) -> Result<Vec<RepositorySymbol>, String> {
        let parsed: CargoToml =
            toml::from_str(content).map_err(|e| format!("failed to parse Cargo.toml: {e}"))?;
        let mut symbols = Vec::new();

        if let Some(pkg) = parsed.package {
            let ver = pkg.version.unwrap_or_else(|| "0.0.0".to_string());
            let edition = pkg.edition.unwrap_or_else(|| "unknown".to_string());
            let sig = format!("package {} v{} (edition {})", pkg.name, ver, edition);
            let id = format!("{file_path}::pkg::{}", pkg.name);

            symbols.push(RepositorySymbol::new(
                id,
                pkg.name.clone(),
                format!("{file_path}::{}", pkg.name),
                SymbolKind::Module,
                file_path,
                1,
                1,
                sig,
                FactClass::VerifiedFact,
                pkg.description,
            ));
        }

        // Dependencies
        for (dep, val) in parsed
            .dependencies
            .into_iter()
            .chain(parsed.dev_dependencies)
            .chain(parsed.build_dependencies)
        {
            let ver_str = match val {
                toml::Value::String(s) => s,
                toml::Value::Table(t) => t
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("*")
                    .to_string(),
                _ => "*".to_string(),
            };
            let id = format!("{file_path}::dep::{dep}");
            symbols.push(RepositorySymbol::new(
                id,
                dep.clone(),
                format!("{file_path}::dep::{dep}"),
                SymbolKind::Variable,
                file_path,
                1,
                1,
                format!("dependency {dep} = \"{ver_str}\""),
                FactClass::VerifiedFact,
                None,
            ));
        }

        // Binary targets
        for bin in parsed.bin {
            if let Some(name) = bin.name {
                let path = bin.path.unwrap_or_default();
                let id = format!("{file_path}::bin::{name}");
                symbols.push(RepositorySymbol::new(
                    id,
                    name.clone(),
                    format!("{file_path}::bin::{name}"),
                    SymbolKind::Function,
                    file_path,
                    1,
                    1,
                    format!("binary target {name} ({path})"),
                    FactClass::VerifiedFact,
                    None,
                ));
            }
        }

        if let Some(lib) = parsed.lib {
            let name = lib.name.unwrap_or_else(|| "lib".to_string());
            let path = lib.path.unwrap_or_default();
            let id = format!("{file_path}::lib::{name}");
            symbols.push(RepositorySymbol::new(
                id,
                name.clone(),
                format!("{file_path}::lib::{name}"),
                SymbolKind::Module,
                file_path,
                1,
                1,
                format!("library target {name} ({path})"),
                FactClass::VerifiedFact,
                None,
            ));
        }

        Ok(symbols)
    }

    pub fn extract_package_json(
        file_path: &str,
        content: &str,
    ) -> Result<Vec<RepositorySymbol>, String> {
        let parsed: PackageJson = serde_json::from_str(content)
            .map_err(|e| format!("failed to parse package.json: {e}"))?;
        let mut symbols = Vec::new();

        if let Some(name) = parsed.name {
            let ver = parsed.version.unwrap_or_else(|| "0.0.0".to_string());
            let id = format!("{file_path}::pkg::{name}");
            symbols.push(RepositorySymbol::new(
                id,
                name.clone(),
                format!("{file_path}::{name}"),
                SymbolKind::Module,
                file_path,
                1,
                1,
                format!("package {name} v{ver}"),
                FactClass::VerifiedFact,
                parsed.description,
            ));
        }

        // Scripts
        for (script_name, script_cmd) in parsed.scripts {
            let id = format!("{file_path}::script::{script_name}");
            symbols.push(RepositorySymbol::new(
                id,
                script_name.clone(),
                format!("{file_path}::script::{script_name}"),
                SymbolKind::Function,
                file_path,
                1,
                1,
                format!("script \"{script_name}\": \"{script_cmd}\""),
                FactClass::VerifiedFact,
                None,
            ));
        }

        // Dependencies
        for (dep, ver) in parsed
            .dependencies
            .into_iter()
            .chain(parsed.dev_dependencies)
        {
            let id = format!("{file_path}::dep::{dep}");
            symbols.push(RepositorySymbol::new(
                id,
                dep.clone(),
                format!("{file_path}::dep::{dep}"),
                SymbolKind::Variable,
                file_path,
                1,
                1,
                format!("dependency \"{dep}\": \"{ver}\""),
                FactClass::VerifiedFact,
                None,
            ));
        }

        Ok(symbols)
    }

    pub fn extract_pyproject(
        file_path: &str,
        content: &str,
    ) -> Result<Vec<RepositorySymbol>, String> {
        let parsed: PyProjectToml =
            toml::from_str(content).map_err(|e| format!("failed to parse pyproject.toml: {e}"))?;
        let mut symbols = Vec::new();

        if let Some(proj) = parsed.project {
            let ver = proj.version.unwrap_or_else(|| "0.0.0".to_string());
            let id = format!("{file_path}::project::{}", proj.name);
            symbols.push(RepositorySymbol::new(
                id,
                proj.name.clone(),
                format!("{file_path}::{}", proj.name),
                SymbolKind::Module,
                file_path,
                1,
                1,
                format!("python project {} v{}", proj.name, ver),
                FactClass::VerifiedFact,
                proj.description,
            ));

            for dep in proj.dependencies {
                let id = format!("{file_path}::dep::{dep}");
                symbols.push(RepositorySymbol::new(
                    id,
                    dep.clone(),
                    format!("{file_path}::dep::{dep}"),
                    SymbolKind::Variable,
                    file_path,
                    1,
                    1,
                    format!("dependency {dep}"),
                    FactClass::VerifiedFact,
                    None,
                ));
            }

            for (name, cmd) in proj.scripts {
                let id = format!("{file_path}::script::{name}");
                symbols.push(RepositorySymbol::new(
                    id,
                    name.clone(),
                    format!("{file_path}::script::{name}"),
                    SymbolKind::Function,
                    file_path,
                    1,
                    1,
                    format!("script \"{name}\" = \"{cmd}\""),
                    FactClass::VerifiedFact,
                    None,
                ));
            }
        } else if let Some(tool) = parsed.tool
            && let Some(poetry) = tool.poetry
        {
            let name = poetry.name.unwrap_or_else(|| "poetry-project".to_string());
            let ver = poetry.version.unwrap_or_else(|| "0.0.0".to_string());
            let id = format!("{file_path}::project::{name}");
            symbols.push(RepositorySymbol::new(
                id,
                name.clone(),
                format!("{file_path}::{name}"),
                SymbolKind::Module,
                file_path,
                1,
                1,
                format!("poetry project {name} v{ver}"),
                FactClass::VerifiedFact,
                poetry.description,
            ));

            for (dep, _) in poetry.dependencies {
                let id = format!("{file_path}::dep::{dep}");
                symbols.push(RepositorySymbol::new(
                    id,
                    dep.clone(),
                    format!("{file_path}::dep::{dep}"),
                    SymbolKind::Variable,
                    file_path,
                    1,
                    1,
                    format!("dependency {dep}"),
                    FactClass::VerifiedFact,
                    None,
                ));
            }

            for (name, cmd) in poetry.scripts {
                let id = format!("{file_path}::script::{name}");
                symbols.push(RepositorySymbol::new(
                    id,
                    name.clone(),
                    format!("{file_path}::script::{name}"),
                    SymbolKind::Function,
                    file_path,
                    1,
                    1,
                    format!("poetry script \"{name}\" = \"{cmd}\""),
                    FactClass::VerifiedFact,
                    None,
                ));
            }
        }

        Ok(symbols)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_manifest_extraction() {
        let toml_content = r#"
[package]
name = "m31a"
version = "0.1.0"
edition = "2024"
description = "Autonomous software engineering runtime"

[dependencies]
syn = { version = "2.0", features = ["full"] }
serde = "1"

[[bin]]
name = "m31a-daemon"
path = "src/main.rs"
"#;

        let symbols = ManifestExtractor::extract_cargo("Cargo.toml", toml_content).unwrap();
        assert!(!symbols.is_empty());

        let pkg = symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Module)
            .unwrap();
        assert_eq!(pkg.name, "m31a");
        assert_eq!(pkg.fact_class, FactClass::VerifiedFact);

        let syn_dep = symbols.iter().find(|s| s.name == "syn").unwrap();
        assert_eq!(syn_dep.kind, SymbolKind::Variable);
        assert_eq!(syn_dep.fact_class, FactClass::VerifiedFact);

        let bin = symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Function)
            .unwrap();
        assert_eq!(bin.name, "m31a-daemon");
    }

    #[test]
    fn test_package_json_extraction() {
        let json_content = r#"
{
  "name": "m31a-dashboard",
  "version": "1.0.0",
  "description": "Frontend UI",
  "scripts": {
    "build": "vite build",
    "test": "vitest"
  },
  "dependencies": {
    "react": "^18.2.0"
  }
}
"#;

        let symbols =
            ManifestExtractor::extract_package_json("package.json", json_content).unwrap();
        let pkg = symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Module)
            .unwrap();
        assert_eq!(pkg.name, "m31a-dashboard");
        assert_eq!(pkg.fact_class, FactClass::VerifiedFact);

        let script = symbols.iter().find(|s| s.name == "build").unwrap();
        assert_eq!(script.kind, SymbolKind::Function);
    }
}
