use std::io::Read;
use std::option::Option;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use tar::Archive as TarArchive;
use url::Url;
use zstd::stream::Decoder as ZstdDecoder;

#[derive(Debug, Deserialize)]
struct PythonJSON {
    apple_sdk_deployment_target: Option<String>,
    crt_features: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug)]
enum InterpreterFlavor {
    Full,
    InstallOnly,
}

#[derive(Debug)]
struct Interpreter {
    implementation: String,
    python_version: String,
    github_release: String,
    triple: String,
    config: String,
    flavor: InterpreterFlavor,
    url: String,
    build_info: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Eq, PartialEq, Hash, Clone)]
struct GroupKey {
    implementation: String,
    python_version: String,
    github_release: String,
    triple: String,
}

#[derive(PartialOrd, Ord, PartialEq, Eq)]
enum ConfigOrder {
    PgoLto,
    Pgo,
    Lto,
    Noopt,
    None,
    Debug,
}

fn read_info_json(url: &str) -> Result<PythonJSON, Box<dyn std::error::Error>> {
    // https://edgarluque.com/blog/zstd-streaming-in-rust/
    let response = ureq::get(url).call()?;

    let decoder = ZstdDecoder::new(response.into_reader()).unwrap();
    let mut archive = TarArchive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap();

        let pathstr = path.to_str().unwrap();
        if pathstr == "python/PYTHON.json" {
            let mut buffer = String::new();
            let _ = entry.read_to_string(&mut buffer);

            let data: PythonJSON = serde_json::from_str(&buffer)?;
            return Ok(data);
        }
    }
    Err("python/PYTHON.json not found".into())
}

fn get_release() -> Result<GitHubRelease, Box<dyn std::error::Error>> {
    let response =
        ureq::get("https://api.github.com/repos/indygreg/python-build-standalone/releases/latest")
            .call()?;
    let release: GitHubRelease = serde_json::from_reader(response.into_reader())?;
    return Ok(release);
}

fn parse_asset(asset: GitHubReleaseAsset) -> Result<Interpreter, Box<dyn std::error::Error>> {
    static INSTALL_ONLY_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^(?P<implementation>\w+)-(?P<pythonVersion>.*)\+(?P<githubRelease>\d{8})-(?P<triple>(?:-?[a-zA-Z0-9_])+)-install_only\.tar\.gz$").unwrap()
    });
    static FULL_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^(?P<implementation>\w+)-(?P<pythonVersion>.*)\+(?P<githubRelease>\d{8})-(?P<triple>(?:-?[a-zA-Z0-9_])+)-(?P<config>debug|pgo\+lto|lto|noopt|pgo)-full.tar.zst$").unwrap()
    });

    let captures = INSTALL_ONLY_RE.captures(&asset.name);
    match captures {
        Some(captures) => {
            let interpreter: Interpreter = Interpreter {
                implementation: captures
                    .name("implementation")
                    .unwrap()
                    .as_str()
                    .to_string(),
                python_version: captures.name("pythonVersion").unwrap().as_str().to_string(),
                github_release: captures.name("githubRelease").unwrap().as_str().to_string(),
                triple: captures.name("triple").unwrap().as_str().to_string(),
                config: "".to_string(),
                flavor: InterpreterFlavor::InstallOnly,
                url: asset.browser_download_url,
                build_info: std::collections::HashMap::new(),
            };
            return Ok(interpreter);
        }
        None => {}
    }

    let captures2 = FULL_RE.captures(&asset.name);
    match captures2 {
        Some(captures2) => {
            let interpreter: Interpreter = Interpreter {
                implementation: captures2
                    .name("implementation")
                    .unwrap()
                    .as_str()
                    .to_string(),
                python_version: captures2
                    .name("pythonVersion")
                    .unwrap()
                    .as_str()
                    .to_string(),
                github_release: captures2
                    .name("githubRelease")
                    .unwrap()
                    .as_str()
                    .to_string(),
                triple: captures2.name("triple").unwrap().as_str().to_string(),
                config: captures2.name("config").unwrap().as_str().to_string(),
                flavor: InterpreterFlavor::Full,
                url: asset.browser_download_url,
                build_info: std::collections::HashMap::new(),
            };
            return Ok(interpreter);
        }
        None => {}
    }

    // TODO: add proper error message
    return Err(format!("{} is not supported", asset.name).into());
}

fn get_config_order(config: &str) -> Result<ConfigOrder, Box<dyn std::error::Error>> {
    match config {
        "pgo+lto" => Ok(ConfigOrder::PgoLto),
        "pgo" => Ok(ConfigOrder::Pgo),
        "lto" => Ok(ConfigOrder::Lto),
        "noopt" => Ok(ConfigOrder::Noopt),
        "" => Ok(ConfigOrder::None),
        "debug" => Ok(ConfigOrder::Debug),
        &_ => Err(format!("Unknown config: {}", config).into()),
    }
}

fn main() {
    let release = get_release().unwrap();

    let mut install_only_assets: Vec<Interpreter> = Vec::new();
    let mut groups: std::collections::HashMap<GroupKey, Vec<Interpreter>> =
        std::collections::HashMap::new();

    for asset in release.assets {
        if !asset.name.ends_with(".tar.zst") && !asset.name.ends_with(".tar.gz") {
            continue;
        }

        let interpreter = parse_asset(asset).unwrap();
        match interpreter.flavor {
            InterpreterFlavor::InstallOnly => {
                install_only_assets.push(interpreter);
            }
            _ => {
                let group_key = GroupKey {
                    implementation: interpreter.implementation.clone(),
                    python_version: interpreter.python_version.clone(),
                    github_release: interpreter.github_release.clone(),
                    triple: interpreter.triple.clone(),
                };
                let exists = groups.get(&group_key);
                if exists.is_none() || exists.unwrap().len() == 0 {
                    groups.insert(group_key, vec![interpreter]);
                } else {
                    groups.get_mut(&group_key).unwrap().push(interpreter);
                }
            }
        }
    }

    for interpreter in install_only_assets {
        println!(
            "{}",
            urlencoding::decode(
                Url::parse(&interpreter.url)
                    .unwrap()
                    .path_segments()
                    .unwrap()
                    .last()
                    .unwrap()
            )
            .unwrap()
        );

        let group_key = GroupKey {
            implementation: interpreter.implementation.clone(),
            python_version: interpreter.python_version.clone(),
            github_release: interpreter.github_release.clone(),
            triple: interpreter.triple.clone(),
        };

        let value: &mut Vec<Interpreter> = groups.get_mut(&group_key).unwrap();
        value.sort_by(|a, b| {
            let order_a = get_config_order(a.config.as_str()).unwrap();
            let order_b = get_config_order(b.config.as_str()).unwrap();

            order_a.cmp(&order_b)
        });

        let best_match: &Interpreter = &value[0];

        println!(
            "  {}",
            urlencoding::decode(
                Url::parse(&best_match.url)
                    .unwrap()
                    .path_segments()
                    .unwrap()
                    .last()
                    .unwrap()
            )
            .unwrap()
        );
        println!("");

        let info = read_info_json(&best_match.url).unwrap();
        println!("{:#?}", info);
    }
}
