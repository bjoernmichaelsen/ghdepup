use hyper_tls::HttpsConnector;
use hyper::Client;
use hyper::Request;
use itertools::Itertools;
use tokio::fs;
use toml::Table;
use std::collections::HashMap;
use std::env;
use std::str;
use std::str::FromStr;
use serde_json::Value;
use semver::Version;

fn get_tag_name(entry: &Value) -> Option<&str> {
    Some(entry.as_object()?["name"].as_str()?)
}
async fn parse_tags_json(json_to_parse: &str) -> Option<Vec<String>> {
    let v : Value = serde_json::from_str(json_to_parse).ok()?;
    let entries = v.as_array()?;
    let str_res = entries
        .iter()
        .filter_map(|e| {get_tag_name(e)});
    Some(str_res
        .into_iter()
        .map(str::to_owned)
        .collect_vec())
}
async fn find_latest_version(versions: Vec<&str>) -> Option<Version> {
    versions
        .into_iter()
        .filter_map(|v| { Version::parse(v).ok()})
        .max()
}
async fn get_repo_tags_json(repo: &str, token: &str) -> Option<String> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    let url = format!("https://api.github.com/repos/{}/tags", repo);
    let req = Request::builder()
        .uri(url)
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {}", token))
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "depup/1.0")
        .body(hyper::Body::empty())
        .unwrap();
    let res = client.request(req).await.ok()?;
    if res.status() != 200 {
        return None;
    }
    let buf = hyper::body::to_bytes(res.into_body()).await.ok()?;
    String::from_utf8(buf.to_vec()).ok()
}
fn get_all_deps(config: &Table) -> Vec<String>{
    const PROJECT_POSTFIX: &str = "_GH_PROJECT";
    return config.keys()
        .filter(|&k| { k.ends_with(PROJECT_POSTFIX)})
        .map(|k| {
            return k[..k.len()-PROJECT_POSTFIX.len()].to_ascii_lowercase()
        })
        .collect_vec();
}
fn get_ghdep_info(config: &Table, dep: &str, key: &str) -> String {
    let key = format!("{}_GH_{}", dep.to_uppercase(), key);
    let value = config
        .get::<String>(&key)
        .unwrap()
        .as_str()
        .unwrap();
    return String::from(value);
}
fn get_project(config: &Table, dep: &str) -> String {
    return get_ghdep_info(config, dep, "PROJECT");
}
fn get_tag_prefix(config: &Table, dep: &str) -> String {
    return get_ghdep_info(config, dep, "TAG_PREFIX");
}
fn get_version_req(config: &Table, dep: &str) -> String {
    return get_ghdep_info(config, dep, "VERSION_REQ");
}
fn get_version(config: &Table, dep: &str) -> String {
    return get_ghdep_info(config, dep, "VERSION");
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    let mut token = env::var("GITHUB_TOKEN").unwrap();
    token.pop();
    Ok(())
}
#[cfg(test)]
mod tests {
    use itertools::join;
    use toml::Table;

    use super::*;
    #[tokio::test]
    async fn test_find_latest_version() {
        let versions = vec!["1.2.3"];
        let latest = find_latest_version(versions).await.unwrap();
        assert_eq!(latest, Version::parse("1.2.3").unwrap());
    }
    #[tokio::test]
    async fn test_parse_tags_json() {
        let json = "
        [
        {
            \"commit\": {
              \"sha\": \"5d305789a86bc9a3d8a352522b219396ad4f3930\",
              \"url\": \"https://api.github.com/repos/bjoernmichaelsen/core/commits/5d305789a86bc9a3d8a352522b219396ad4f3930\"
            },
            \"name\": \"1.0.0\",
            \"node_id\": \"MDM6UmVmMzI4ODUxNTQ6cmVmcy90YWdzL3N1c2UtNC4wLTE=\",
            \"tarball_url\": \"https://api.github.com/repos/bjoernmichaelsen/core/tarball/refs/tags/suse-4.0-1\",
            \"zipball_url\": \"https://api.github.com/repos/bjoernmichaelsen/core/zipball/refs/tags/suse-4.0-1\"
        },
        {
            \"commit\": {
              \"sha\": \"5d305789a86bc9a3d8a352522b219396ad4f3931\",
              \"url\": \"https://api.github.com/repos/bjoernmichaelsen/core/commits/5d305789a86bc9a3d8a352522b219396ad4f3931\"
            },
            \"name\": \"1.2.3\",
            \"node_id\": \"MDM6UmVmMzI4ODUxNTQ6cmVmcy90YWdzL3N1c2UtNC4wLTF=\",
            \"tarball_url\": \"https://api.github.com/repos/bjoernmichaelsen/core/tarball/refs/tags/suse-4.0-2\",
            \"zipball_url\": \"https://api.github.com/repos/bjoernmichaelsen/core/zipball/refs/tags/suse-4.0-2\"
        }
        ]";
        let expected = "1.0.0, 1.2.3";
        let actual = parse_tags_json(json).await.expect("This should parse.");
        assert_eq!(join(actual.iter(), ", "), expected);
    }
    static CONFIG_CONTENT : &str = "
# this config should be kept parsable by POSIX sh, make, ini and toml
HYPER_GH_PROJECT=\"hyperium/hyper\"
HYPER_GH_TAG_PREFIX=\"v\"
HYPER_GH_VERSION_REQ=\">=0.14, <1\"
HYPER_GH_VERSION=\"0.14.26\"
    ";
    #[tokio::test]
    async fn test_parse_config() {
        let config = toml::from_str::<Table>(CONFIG_CONTENT)
            .expect("should parse");
        assert_eq!(
            get_project(&config, "hyper").as_str(),
            "hyperium/hyper");
        assert_eq!(
            get_tag_prefix(&config, "hyper").as_str(),
            "v");
        assert_eq!(
            get_version_req(&config, "hyper").as_str(),
            ">=0.14, <1");
        assert_eq!(
            get_version(&config, "hyper").as_str(),
            "0.14.26");
        assert_eq!(
            join(get_all_deps(&config), ", "),
            "hyper"
        );   
    }
}