use std::{env, fs, str::FromStr, collections::HashMap};

use toml::Value;

fn read_versions(path: &str) -> std::collections::HashMap<String, String> {
    let filecontents = fs::read_to_string(path).expect("could not read versions");
    let table = toml::Table::from_str(filecontents.as_str()).expect("could not parse versions");
    table.into_iter()
        .map(|e| { (e.0, e.1.as_str().expect("cant parse version").to_owned()) })
        .collect()
}
fn update_entry_version(entry: &mut toml::Table, version: &str) {
    const VERSION_KEY : &str = "version";
    if !entry.contains_key(VERSION_KEY) {
        return;
    }
    entry.insert(VERSION_KEY.to_owned(), toml::Value::String(version.to_owned()));
}
fn update_cargo(path: &str, versions: &HashMap<String, String>) {
    const DEPENDENCIES_KEY : &str = "dependencies";
    let filecontents = fs::read_to_string(path).expect("could not read cargofile");
    let mut table = toml::Table::from_str(filecontents.as_str()).expect("could not parse cargofile");
    let deps = table
        .get_mut(DEPENDENCIES_KEY)
        .and_then(Value::as_table_mut);
    deps.map(|ds|{
        versions.iter()
            .for_each(|new_entry| {
                let entry = ds.get_mut(new_entry.0).and_then(Value::as_table_mut);
                if let Some(e) = entry {
                    update_entry_version(e, new_entry.1);
                }
            })
    });
    println!("cargo at {}, modified with {:?}: {:#?}", path, versions, table);
}
fn main() {
    let usage = "usage: selfupdate ghversions.toml ./Cargo.toml ./somewhere/Cargo.toml ...";
    let versionsfile = env::args().nth(1).expect(usage);
    let cargofiles: Vec<String> = env::args().skip(2).collect();
    let versions = read_versions(versionsfile.as_str());
    cargofiles.iter()
        .for_each(|c| {update_cargo(c, &versions)})
}

#[cfg(test)]
mod tests {
    use super::*;
}