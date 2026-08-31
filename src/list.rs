use std::error::Error;
use std::io::{self, Write};

use reqwest::blocking::Client;
use reqwest::header::ACCEPT;
use serde::Deserialize;

const RELEASES_URL: &str = "https://api.github.com/repos/AcademySoftwareFoundation/rez/releases";
const PAGE_SIZE: usize = 100;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
}

pub fn run(json: bool) -> Result<(), Box<dyn Error>> {
    let client = crate::http::client()?;
    let versions = fetch_rez_versions(&client, RELEASES_URL)?;
    let output = render_versions(&versions, json)?;

    io::stdout().lock().write_all(output.as_bytes())?;
    Ok(())
}

fn fetch_rez_versions(client: &Client, url: &str) -> Result<Vec<String>, reqwest::Error> {
    let mut versions = Vec::new();
    let mut page = 1;

    loop {
        let releases = client
            .get(url)
            .header(ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .query(&[("per_page", PAGE_SIZE), ("page", page)])
            .send()?
            .error_for_status()?
            .json::<Vec<Release>>()?;
        let is_last_page = releases.len() < PAGE_SIZE;

        versions.extend(
            releases
                .into_iter()
                .filter(|release| !release.draft)
                .map(|release| release.tag_name),
        );

        if is_last_page {
            return Ok(versions);
        }
        page += 1;
    }
}

fn render_versions(versions: &[String], json: bool) -> Result<String, serde_json::Error> {
    if json {
        serde_json::to_string(versions).map(|output| output + "\n")
    } else if versions.is_empty() {
        Ok(String::new())
    } else {
        Ok(versions.join("\n") + "\n")
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    #[test]
    fn fetches_all_pages_and_omits_drafts() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let first_page = (0..PAGE_SIZE)
                .map(|version| format!(r#"{{"tag_name":"3.{version}.0","draft":false}}"#))
                .collect::<Vec<_>>()
                .join(",");
            let second_page = r#"[
                {"tag_name":"2.0.0","draft":false},
                {"tag_name":"future","draft":true},
                {"tag_name":"1.0.0","draft":false}
            ]"#
            .to_owned();

            for (index, body) in [format!("[{first_page}]"), second_page]
                .into_iter()
                .enumerate()
            {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 1024];
                let request_length = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..request_length]);
                assert!(
                    request.starts_with(&format!("GET /releases?per_page=100&page={} ", index + 1))
                );
                assert!(request.contains("accept: application/vnd.github+json"));
                assert!(request.contains(concat!("user-agent: rezup/", env!("CARGO_PKG_VERSION"))));

                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });

        let client = crate::http::client().unwrap();
        let versions = fetch_rez_versions(&client, &format!("http://{address}/releases")).unwrap();
        server.join().unwrap();

        assert_eq!(versions.len(), PAGE_SIZE + 2);
        assert_eq!(versions.first().unwrap(), "3.0.0");
        assert_eq!(versions.last().unwrap(), "1.0.0");
        assert!(!versions.iter().any(|version| version == "future"));
    }

    #[test]
    fn renders_text_and_json() {
        let versions = vec!["3.4.0".to_owned(), "3.3.0".to_owned()];

        assert_eq!(render_versions(&versions, false).unwrap(), "3.4.0\n3.3.0\n");
        assert_eq!(
            render_versions(&versions, true).unwrap(),
            "[\"3.4.0\",\"3.3.0\"]\n"
        );
    }
}
