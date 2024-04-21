use std::io::Read;
use std::option::Option;

use futures::{StreamExt, TryFutureExt, TryStreamExt};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use url::Url;

/// A short version of what's in the PYTHON.json file.
#[derive(Debug, Deserialize, Clone)]
struct PythonJSON {
    apple_sdk_deployment_target: Option<String>,
    crt_features: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
}

/// A GitHub release.
#[derive(Deserialize)]
struct GitHubRelease {
    assets: Vec<GitHubReleaseAsset>,
}

/// python-standalone-build provides two types of archives: install_only and full.
#[derive(Debug, Clone)]
enum InterpreterFlavor {
    Full,
    InstallOnly,
}

/// A Python interpreter from python-standalone-build.
#[derive(Debug, Clone)]
struct Interpreter {
    implementation: String,
    python_version: String,
    github_release: String,
    triple: String,
    config: String,
    flavor: InterpreterFlavor,
    url: String,
    info: Option<PythonJSON>,
    interpreter_implemented: Option<Box<Interpreter>>,
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

async fn get_release(client: &reqwest::Client) -> Result<GitHubRelease, reqwest::Error> {
    return client
        .get("https://api.github.com/repos/indygreg/python-build-standalone/releases/latest")
        .send()
        .await?
        .json::<GitHubRelease>()
        .await;
}

fn parse_asset(asset: GitHubReleaseAsset) -> anyhow::Result<Interpreter> {
    static INSTALL_ONLY_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^(?P<implementation>\w+)-(?P<pythonVersion>.*)\+(?P<githubRelease>\d{8})-(?P<triple>(?:-?[a-zA-Z0-9_])+)-install_only\.tar\.gz$").unwrap()
    });

    let captures = INSTALL_ONLY_RE.captures(&asset.name);
    if let Some(caps) = captures {
        return Ok(Interpreter {
            implementation: caps.name("implementation").unwrap().as_str().to_string(),
            python_version: caps.name("pythonVersion").unwrap().as_str().to_string(),
            github_release: caps.name("githubRelease").unwrap().as_str().to_string(),
            triple: caps.name("triple").unwrap().as_str().to_string(),
            config: "".to_string(),
            flavor: InterpreterFlavor::InstallOnly,
            url: asset.browser_download_url,
            info: None,
            interpreter_implemented: None,
        });
    }

    static FULL_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^(?P<implementation>\w+)-(?P<pythonVersion>.*)\+(?P<githubRelease>\d{8})-(?P<triple>(?:-?[a-zA-Z0-9_])+)-(?P<config>debug|pgo\+lto|lto|noopt|pgo)-full.tar.zst$").unwrap()
    });

    let captures = FULL_RE.captures(&asset.name);
    if let Some(caps) = captures {
        return Ok(Interpreter {
            implementation: caps.name("implementation").unwrap().as_str().to_string(),
            python_version: caps.name("pythonVersion").unwrap().as_str().to_string(),
            github_release: caps.name("githubRelease").unwrap().as_str().to_string(),
            triple: caps.name("triple").unwrap().as_str().to_string(),
            config: caps.name("config").unwrap().as_str().to_string(),
            flavor: InterpreterFlavor::Full,
            url: asset.browser_download_url,
            info: None,
            interpreter_implemented: None,
        });
    }

    // TODO: add proper error message
    return Err(anyhow::anyhow!("{} is not supported", asset.name));
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

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let client = reqwest::Client::new();

    let release = get_release(&client)
        .unwrap_or_else(|error| {
            eprintln!("Failed to get release: {:?}", error);
            std::process::exit(1);
        })
        .await;

    let mut install_only_interpreters: Vec<Interpreter> = Vec::new();
    let mut groups: std::collections::HashMap<GroupKey, Vec<Interpreter>> =
        std::collections::HashMap::new();

    for asset in release.assets {
        if asset.name.ends_with(".tar.zst") || asset.name.ends_with(".tar.gz") {
            let interpreter = parse_asset(asset).unwrap_or_else(|error| {
                eprintln!("Failed to get asset: {:?}", error);
                std::process::exit(1);
            });

            match interpreter.flavor {
                InterpreterFlavor::InstallOnly => {
                    install_only_interpreters.push(interpreter);
                }
                InterpreterFlavor::Full => {
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
    }

    for mut interpreter in &mut install_only_interpreters {
        // println!(
        //     "{}",
        //     urlencoding::decode(
        //         Url::parse(&interpreter.url)
        //             .unwrap()
        //             .path_segments()
        //             .unwrap()
        //             .last()
        //             .unwrap()
        //     )
        //     .unwrap()
        // );

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

        // println!(
        //     "  {}",
        //     urlencoding::decode(
        //         Url::parse(&best_match.url)
        //             .unwrap()
        //             .path_segments()
        //             .unwrap()
        //             .last()
        //             .unwrap()
        //     )
        //     .unwrap()
        // );
        // let info = read_info_json(&best_match.url).unwrap();
        // interpreter.info = Some(info.clone());
        // println!("  {:?}", info);

        interpreter.interpreter_implemented = Some(Box::new(best_match.clone()));
        // println!("");
    }

    // use rayon::iter::ParallelIterator;
    // use rayon::prelude::IntoParallelIterator;
    //
    // let pool = rayon::ThreadPoolBuilder::new()
    //     .num_threads(1)
    //     .build()
    //     .unwrap();
    // pool.install(|| {
    //     let par_iter = install_only_interpreters[1..2]
    //         .into_par_iter()
    //         .map(|interpreter| read_info_json(&interpreter.url));
    //     let data: Vec<_> = par_iter.collect();
    // })

    // Try with futures: https://stackoverflow.com/questions/51044467/how-can-i-perform-parallel-asynchronous-http-get-requests-with-reqwest
    let asd = futures::stream::iter(install_only_interpreters)
        .map(|interpreter| {
            let client = &client;
            async move {
                let interpreter_implemented = interpreter.interpreter_implemented.clone().unwrap();
                let info = read_info_json(&client, interpreter_implemented.url)
                    .await
                    .unwrap();
                println!("  {:?}", info);
            }
        })
        .buffer_unordered(20);
    asd.for_each(|b| async {}).await;
}

// https://github.com/astral-sh/uv/blob/main/crates/uv-extract/src/stream.rs#L154
async fn read_info_json(client: &reqwest::Client, url: String) -> anyhow::Result<PythonJSON> {
    println!("Reading info from {}", url);
    // https://edgarluque.com/blog/zstd-streaming-in-rust/
    let response = client.get(url).send().await.unwrap();

    let reader = response
        .bytes_stream()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
        .into_async_read()
        .compat();

    return read_info_json_entry(reader).await;
}

async fn read_info_json_entry<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
) -> anyhow::Result<PythonJSON> {
    let reader = tokio::io::BufReader::new(reader);
    let decompressed_bytes = async_compression::tokio::bufread::ZstdDecoder::new(reader);

    let mut archive = tokio_tar::ArchiveBuilder::new(decompressed_bytes).build();
    let mut entries = archive.entries().unwrap();
    let mut pinned = std::pin::Pin::new(&mut entries);

    while let Some(entry) = pinned.next().await {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap();

        let pathstr = path.to_str().unwrap();
        if pathstr == "python/PYTHON.json" {
            let mut buffer = String::new();

            let _ = entry.read_to_string(&mut buffer).await.unwrap();

            let data: PythonJSON = serde_json::from_str(&buffer).unwrap();
            return Ok(data);
        }
    }
    return Err(anyhow::anyhow!("Could not find PYTHON.json"));
}
