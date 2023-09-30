use futures::future::join_all;
use hyper_tls::HttpsConnector;
use hyper::Client;
use hyper::Request;
use itertools::Itertools;
use semver::VersionReq;
use toml::Table;
use std::env;
use std::str;
use std::str::FromStr;
use std::string::FromUtf8Error;
use serde_json::Value;
use semver::Version;

enum DepKey {
    Project,
    TagPrefix,
    Version,
    VersionReq
}
impl DepKey {
    fn as_str(&self) -> &'static str {
        match self {
            DepKey::Project => "PROJECT",
            DepKey::TagPrefix => "TAG_PREFIX",
            DepKey::Version => "VERSION",
            DepKey::VersionReq => "VERSION_REQ"
        }
    }
    fn as_full_postfix(&self) -> String {
        const GH_PREFIX : &str = "GH";
        return format!("{}_{}", GH_PREFIX, self.as_str());
    }
}
trait GhDepContainer {
    fn get_ghdep_info(&self, dep: &str, key: DepKey) -> Option<String>;
    fn get_all_deps(&self) -> Vec<String>;
}
impl GhDepContainer for Table {
    fn get_ghdep_info(&self, dep: &str, key: DepKey) -> Option<String> {
        let key = format!("{}_{}", dep.to_uppercase(), key.as_full_postfix().as_str());
        let value = self
            .get::<String>(&key)?
            .as_str()?;
        return Some(value.to_string());
    }
    fn get_all_deps(&self) -> Vec<String>{
        let postfix = DepKey::Project.as_full_postfix();
        let chars_to_remove = postfix.len() + 1;
        return self.keys()
            .filter(|&k| { k.ends_with(postfix.as_str()) })
            .map(|k| {
                return k[..k.len()-chars_to_remove].to_ascii_lowercase()
            })
            .collect_vec();
    }
}
struct Dep {
    name: String,
    project: String,
    version_req: Option<VersionReq>,
    current_version: Option<Version>,
    tag_prefix: String,
    available_tags: Vec<String>,
    available_versions: Vec<Version>,
    best_version: Option<Version>
}
impl Dep {
    fn from_table(table: &Table, dep: &str) -> Self {
        let v = Version::from_str(table.get_ghdep_info(
            dep,
            DepKey::Version)
            .unwrap_or_default()
            .as_str()).ok();
        let vr = VersionReq::from_str(table.get_ghdep_info(
            dep,
            DepKey::VersionReq)
            .unwrap_or_default()
            .as_str()).ok();
        return Self {
            name: dep.to_string(),
            project: table.get_ghdep_info(dep, DepKey::Project).unwrap_or_default(),
            version_req: vr,
            current_version: v,
            tag_prefix: table.get_ghdep_info(dep, DepKey::TagPrefix).unwrap_or_default(),
            available_tags: vec![],
            available_versions: vec![],
            best_version: None
        }
    }
    fn get_versions_from_tags(tags: &Vec<String>, tag_prefix: &str) -> Vec<Version> {
        tags.iter().filter_map(|tag| {
            if !tag.starts_with(tag_prefix) {
                return None;
            }
            Version::parse(&tag[tag_prefix.len()..tag.len()]).ok()
        })
        .collect_vec()
    }
    fn update_versions_from_tags(&mut self) {
        self.available_versions = Dep::get_versions_from_tags(
            &self.available_tags,
            self.tag_prefix.as_str());
    }
    fn get_best_version(versions: &Vec<Version>, version_req: &Option<VersionReq>) -> Option<Version> {
        versions.iter()
            .filter(|&v| {
                match &version_req {
                    None => true,
                    Some(vr) => vr.matches(v)
                }
            })
            .max()
            .and_then(|v| Some(v.clone()))
    }
    fn update_best_version(&mut self) {
        self.best_version = Dep::get_best_version(
            &self.available_versions,
            &self.version_req);
    }
}
impl std::fmt::Display for Dep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_VERSION=\"{}\"",
            self.name.to_ascii_uppercase(),
            self.best_version.as_ref().map(|v| v.to_string()).unwrap_or_default()
        )
    }
}
impl std::fmt::Debug for Dep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let available_versions = self.available_versions.iter()
            .map(|v| v.to_string())
            .collect_vec();
        write!(f, "# {}
# from {}
# previous version: {}
# with tags: {}
# with versions: {}
{}_VERSION=\"{}\"
",
            self.name,
            self.project,
            self.current_version.as_ref().map(|v| v.to_string()).unwrap_or_default(),
            self.available_tags.join(", "),
            available_versions.join(", "),
            self.name.to_ascii_uppercase(),
            &self.best_version.as_ref().map(|v| v.to_string()).unwrap_or_default()
        )
    }
}
enum ConfigError {
    TooFewArgs(usize),
    NoOutputFile(),
    ConfigReadError(String),
    FromUtf8Error(),
    TomlParseError(toml::de::Error),
    GithubTokenMissing()
}
impl std::fmt::Debug for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted = match self {
            Self::TooFewArgs(args) => format!("at least two config files needed, but only {} found.", args),
            Self::NoOutputFile() => format!("no output file."),
            Self::ConfigReadError(filename) => format!("error reading config file: {}.", filename),
            Self::FromUtf8Error() => format!("config is not valid utf8."),
            Self::TomlParseError(e) => format!("config cant be parsed as toml: {}", e),
            Self::GithubTokenMissing() => format!("GITHUB_TOKEN environment variable is missing or unset.")
        };
        write!(f, "{}", formatted)
    }
}
impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for ConfigError {}

async fn setup_config(args: &Vec<String>) -> Result<(toml::Table, String), ConfigError> {
    let input_file_names = args.iter()
        .skip(1)
        .collect_vec();
    if input_file_names.len() < 2 {
        return Err(ConfigError::TooFewArgs(input_file_names.len()));
    };
    let output_file_name = args
        .last()
        .ok_or(ConfigError::NoOutputFile())?
        .clone();
    let mut buf = vec![];
    for &f in input_file_names.iter() {
        let mut c = tokio::fs::read(f).await
            .or(Err(ConfigError::ConfigReadError(f.clone())))?
            .clone();
        buf.append(c.as_mut());
        buf.push(b'\n');
    }
    let config_str = String::from_utf8(buf)
        .or(Err(ConfigError::FromUtf8Error()))?;
    let config = toml::from_str::<Table>(config_str.as_str())
        .or_else(|e| Err(ConfigError::TomlParseError(e)))?;
    Ok((config, output_file_name))
}

enum GetTagsError {
    ExpectedJsonArrayError(),
    ExpectedJsonName(),
    ExpectedJsonObjectError(),
    FromUtf8Error(FromUtf8Error),
    HyperError(hyper::Error),
    HyperHttpError(hyper::http::Error),
    HyperHttpStatusError(hyper::http::StatusCode),
    JsonParseError(),
    MultipleGithubErrors(Vec<String>)
}
impl std::fmt::Debug for GetTagsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted = match self {
            Self::ExpectedJsonArrayError() => format!("json array not found where expected in response."),
            Self::ExpectedJsonName() => format!("name not found where expected in json response."),
            Self::ExpectedJsonObjectError() => format!("object not found where expected in json response."),
            Self::FromUtf8Error(e) => format!("error parsing response as UTF8: {}.", e),
            Self::HyperError(e) => format!("hyper error: {}.", e),
            Self::HyperHttpError(e) => format!("hyper http error: {}.", e),
            Self::HyperHttpStatusError(e) => format!("unexpected http status: {}.", e),
            Self::JsonParseError() => format!("error parsing json response"),
            Self::MultipleGithubErrors(errs) => {
                errs.iter()
                    .map(|e| format!("{}", e))
                    .join("\n")
            }
        };
        write!(f, "{}", formatted)
    }
}
impl std::fmt::Display for GetTagsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for GetTagsError {}

fn get_tag_name(entry: &Value) -> Result<&str, GetTagsError> {
    entry
        .as_object()
        .ok_or(GetTagsError::ExpectedJsonObjectError())?
        .get("name")
        .ok_or(GetTagsError::ExpectedJsonName())?
        .as_str()
        .ok_or(GetTagsError::ExpectedJsonName())
}
async fn parse_tags_json(json_to_parse: &str) -> Result<Vec<String>, GetTagsError> {
    let v : Value = serde_json::from_str(json_to_parse)
        .map_err(|_| GetTagsError::JsonParseError())?;
    let entries = v.as_array()
        .ok_or(GetTagsError::ExpectedJsonArrayError())?;
    let str_res = entries
        .iter()
        .filter_map(|e| {get_tag_name(e).ok()});
    Ok(str_res
        .into_iter()
        .map(str::to_owned)
        .collect_vec())
}
async fn get_repo_tags_json(project: &str, token: &str) -> Result<String, GetTagsError> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    let url = format!("https://api.github.com/repos/{}/tags", project);
    let req = Request::builder()
        .uri(url)
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {}", token))
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "ghdepup/1.0")
        .body(hyper::Body::empty())
        .map_err(|e| GetTagsError::HyperHttpError(e))?;
    let res = client.request(req)
        .await
        .map_err(|e| GetTagsError::HyperError(e))?;
    return match res.status().is_success() {
        false => { Err(GetTagsError::HyperHttpStatusError(res.status())) },
        true => {
            let body = res.into_body();
            let buf = hyper::body::to_bytes(body)
                .await
                .map_err(|e| GetTagsError::HyperError(e))?;
            String::from_utf8(buf.to_vec())
                .map_err(|e| GetTagsError::FromUtf8Error(e))
        }
    }
}
async fn update_tags_from_gh(dep: &mut Dep, token: &str) -> Result<(), GetTagsError> {
    let json = get_repo_tags_json(dep.project.as_str(), token)
        .await?;
    let tags = parse_tags_json(json.as_str())
        .await?;
    dep.available_tags = tags;
    Ok(())
}
#[cfg(feature="print_debug")]
async fn print_debug(deps: &Vec<Dep>) {
    deps.iter().for_each(|dep| {
        println!("{:?}", dep);
    });
}
#[cfg(not(feature="print_debug"))]
async fn print_debug(_: &Vec<Dep>) {}

#[cfg(feature="write_outfile")]
async fn write_outfile(deps: &Vec<Dep>, outfile: &str) {
    let formatted = deps.iter()
        .map(|d| format!("{}\r\n", d))
        .reduce(|acc, el| acc + el.as_str())
        .unwrap_or("".to_string());
    tokio::fs::write(outfile, formatted.as_bytes())
        .await
        .expect("fatal: unable to write updated file.")
}
#[cfg(not(feature="write_outfile"))]
async fn write_outfile(_: &Vec<Dep>, _: &str) {}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    let mut token = env::var("GITHUB_TOKEN")
        .or(Err(Box::new(ConfigError::GithubTokenMissing()) as Box<dyn std::error::Error>))?;
    token.pop();
    let config_result : Result<(toml::Table, String), ConfigError>= setup_config(&env::args().collect_vec()).await;
    let (config, outfile) = config_result.unwrap();
    let mut deps = config.get_all_deps().iter()
        .map(|depname| Dep::from_table(&config, depname))
        .collect_vec();
    let updates = deps.iter_mut().map(|dep| {
        update_tags_from_gh(dep, token.as_str())
    });
    let updates = join_all(updates).await.into_iter().collect_vec();
    if !updates.iter().all(|result| result.is_ok()) {
        let e : Box<dyn std::error::Error> = Box::new(GetTagsError::MultipleGithubErrors(
            updates.iter()
                .filter_map(|r|{
                    r.as_ref().err().map(|e| format!("{}", e))
                })
                .collect_vec()
        ));
        return Err(e);
    }
    deps.iter_mut().for_each(|dep| {
        dep.update_versions_from_tags();
        dep.update_best_version();
    });
    print_debug(&deps).await;
    write_outfile(&deps, outfile.as_str()).await;
    Ok(())
}
#[cfg(test)]
mod tests {
    use itertools::join;
    use toml::Table;

    use super::*;
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
        let actual = parse_tags_json(json).await.ok().expect("this should parse");
        assert_eq!(join(actual.iter(), ", "), expected);
    }
    static CONFIG_CONTENT : &str = "
# this config should be kept parsable by POSIX sh, make, ini and toml
HYPER_GH_PROJECT=\"hyperium/hyper\"
HYPER_GH_TAG_PREFIX=\"v\"
HYPER_GH_VERSION_REQ=\">=0.14, <1\"
HYPER_GH_VERSION=\"0.14.26\"

HYPER_TLS_GH_PROJECT=\"hyperium/hyper-tls\"
HYPER_TLS_GH_TAG_PREFIX=\"v\"
HYPER_TLS_GH_VERSION_REQ=\">=0.5\"
HYPER_TLS_GH_VERSION=\"0.5.0\"
    ";
    #[tokio::test]
    async fn test_parse_config() {
        let config = toml::from_str::<Table>(CONFIG_CONTENT)
            .expect("should parse");
        assert_eq!(
            join(config.get_all_deps(), ", "),
            "hyper, hyper_tls"
        );
        let hyper = Dep::from_table(&config, "hyper");
        assert_eq!(hyper.name, "hyper");
        assert_eq!(hyper.project, "hyperium/hyper");
        assert_eq!(hyper.tag_prefix, "v");
        assert_eq!(hyper.version_req, VersionReq::parse(">=0.14, <1").ok());
        assert_eq!(hyper.available_versions.len(), 0);
        assert_eq!(hyper.available_tags.len(), 0);
        assert_eq!(
            hyper.current_version,
            Some(Version::parse("0.14.26").expect("should parse")));
        assert_eq!(hyper.best_version, None)
    }
    #[tokio::test]
    async fn test_get_versions_from_tags() {
        let tags =vec![
                "foo111",
                "bar2",
                "v1.2.3",
                "2.3.4",
                "v3.4.5"].iter().map(|&s| {s.to_string()}).collect_vec();
        let versions = Dep::get_versions_from_tags(&tags, "v");
        assert_eq!(
            join(versions, ", "),
            "1.2.3, 3.4.5");
    }
    #[tokio::test]
    async fn test_get_best_version() {
        let versions =vec![
                "1.2.3",
                "3.4.0",
                "3.4.5",
                "4.5.6"].iter().map(|&s| {Version::parse(s).unwrap()}).collect_vec();
        let version_req = VersionReq::from_str(">=3, <4").ok();
        assert_eq!(
            Dep::get_best_version(&versions, &version_req),
            Version::parse("3.4.5").ok());
    }
}
