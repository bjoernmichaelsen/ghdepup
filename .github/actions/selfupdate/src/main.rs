use std::{env, fs, str::FromStr, collections::HashMap};
use std::io::Write;

use toml::Value;

struct GhdepupFileContents {
    ghdeps_contents : String,
    ghversions_contents : String,
}

fn get_ghdepup_contents(ghdeps_filename: &str, ghversions_filename: &str, contents: &mut GhdepupFileContents) {
    contents.ghdeps_contents = fs::read_to_string(ghdeps_filename).expect("could not read ghdeps.");
    contents.ghversions_contents = fs::read_to_string(ghversions_filename).expect("could not read ghversions.");
}

fn get_crate_versions(ghdepup_contents: &GhdepupFileContents, versions_by_crate: &mut std::collections::HashMap<String, String>) {
    let deps_table = toml::Table::from_str(ghdepup_contents.ghdeps_contents.as_str()).expect("could not parse ghdeps");
    let versions_table = toml::Table::from_str(ghdepup_contents.ghversions_contents.as_str()).expect("could not parse ghversions");
    deps_table.into_iter()
        .for_each(|e| {
            let maybe_version_value = e.0.strip_suffix("_CRATE_NAME")
                .map(|n| format!("{}_GH_VERSION", n))
                .map(|vk| versions_table.get(&vk))
                .flatten();
            if let Some(version_value) = maybe_version_value {
                versions_by_crate.insert(
                    e.1.as_str().unwrap().to_string(),
                    version_value.as_str().unwrap().to_string());
            }
        });
}

fn update_entry_version(entry: &mut toml::Table, new_version: &str) {
    const VERSION_KEY : &str = "version";
    entry.insert(VERSION_KEY.to_owned(), toml::Value::String(new_version.to_owned()));
}

fn update_deps(deps: &mut toml::map::Map<String, Value>, versions: &HashMap<String, String>) {
    versions.iter().for_each(|new_entry| {
        let entry = deps.get_mut(new_entry.0).and_then(Value::as_table_mut);
        println!("entry found for dep {}: {:#?}", new_entry.0, entry);
        if let Some(e) = entry {
            update_entry_version(e, new_entry.1);
            deps[new_entry.0] = Value::from(e.to_owned());
        }
    });
}

fn update_cargo(path: &str, versions: &HashMap<String, String>) {
    const DEPENDENCIES_KEY : &str = "dependencies";
    let filecontents = fs::read_to_string(path).expect("could not read cargofile");
    let mut table = toml::Table::from_str(filecontents.as_str()).expect("could not parse cargofile");
    let mut deps = table
        .get(DEPENDENCIES_KEY)
        .and_then(Value::as_table)
        .expect("missing dependencies in Cargo file")
        .to_owned();
    update_deps(&mut deps, versions);
    table[DEPENDENCIES_KEY] = Value::from(deps.clone());
    let out_cargo = toml::ser::to_string_pretty(&table).ok().unwrap();
    println!("cargo at {}, modified with {:?}: toml::: {}", path, versions, out_cargo);
    let mut outfile = fs::File::create(path).ok().unwrap();
    outfile.write_all(out_cargo.as_bytes()).ok().unwrap();
}

fn main() {
    let usage = "usage: selfupdate ghdeps.toml ghversions.toml ./Cargo.toml ./somewhere/Cargo.toml ...";
    let depsfile = env::args().nth(1).expect(usage);
    let versionsfile = env::args().nth(2).expect(usage);
    let mut contents = GhdepupFileContents {
        ghdeps_contents: String::new(),
        ghversions_contents: String::new()
    };
    get_ghdepup_contents(depsfile.as_str(), versionsfile.as_str(), &mut contents);
    let mut crate_versions = std::collections::HashMap::new();
    get_crate_versions(&contents, &mut crate_versions);
    let cargofiles: Vec<String> = env::args().skip(3).collect();
    cargofiles.iter()
        .for_each(|c| {update_cargo(c, &crate_versions);});
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_crate_versions() {
        let deps = "HYPER_CRATE_NAME=\"hyper\"\n".to_string();
        let versions = "HYPER_GH_VERSION=\"1.2.3\"\n".to_string();
        let contents = GhdepupFileContents {
            ghdeps_contents: deps,
            ghversions_contents: versions
        };
        let mut crate_versions = std::collections::HashMap::new();
        get_crate_versions(&contents, &mut crate_versions);
        assert_eq!(crate_versions.get("hyper").unwrap(), "1.2.3")
    }
    #[test]
    fn test_update_deps() {
        let toml_raw = r#"[dependencies]
hyper = { version = "0.14", features = ["full"] }
hyper-tls = { version = "0.5.0" }
tokio = { version = "1", features = ["full", "rt", "macros"] }
serde = { version = "1.0" }
serde_json = { version = "1.0", features = ["std"] }
itertools = { version = "0.11.0" }
semver =  { version = "1.0.18" }
toml = { version = "0.8.0", features = ["parse", "display"] }
futures = { version = "0.3.5", features = ["std"] }"#;
        let table = toml::Table::from_str(toml_raw).expect("could not parse toml");
        const DEPENDENCIES_KEY : &str = "dependencies";
        let mut deps = table
            .get(DEPENDENCIES_KEY)
            .and_then(Value::as_table)
            .expect("couldnt find deps in toml")
            .to_owned();
        let versions = HashMap::from([
            ("hyper".to_string(), "42".to_string()),
            ("hyper-tls".to_string(), "43".to_string()),
            ("tokio".to_string(), "44".to_string()),
            ("serde".to_string(), "45".to_string()),
            ("serde_json".to_string(), "46".to_string()),
            ("itertools".to_string(), "47".to_string()),
            ("semver".to_string(), "48".to_string()),
            ("toml".to_string(), "49".to_string()),
            ("futures".to_string(), "50".to_string())
        ]);
        update_deps(&mut deps, &versions);
        let deps_out = toml::ser::to_string(&deps).ok().unwrap();
        let expected_deps_out = r#"[futures]
features = ["std"]
version = "50"

[hyper]
features = ["full"]
version = "42"

[hyper-tls]
version = "43"

[itertools]
version = "47"

[semver]
version = "48"

[serde]
version = "45"

[serde_json]
features = ["std"]
version = "46"

[tokio]
features = ["full", "rt", "macros"]
version = "44"

[toml]
features = ["parse", "display"]
version = "49"
"#;
        assert_eq!(deps_out, expected_deps_out);
    }
}
