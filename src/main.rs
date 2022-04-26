use std::collections::HashMap;

use anyhow::Result;
use chrono::DateTime;
use futures::{
    future::ready,
    stream::{self, StreamExt},
};
use reqwest::Client;

use tokio::runtime::Builder;
use tracing::{info, info_span, instrument, debug_span, Instrument};
use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

const CONCURRENT_REQUESTS: usize = 10;

#[derive(Debug, Default)]
struct Stats {
    total: usize,
    success: usize,
    npe2: usize,
    stragglers: Vec<String>,
}

#[instrument]
async fn analyze_plugin(client: &Client, repo: &str, branch: &str) -> Result<(bool, bool)> {
    let setups = ["setup.cfg", "setup.py"];
    let mut any = false;
    for setup in setups {
        let url = format!(
            "https://raw.githubusercontent.com/{repo}/{branch}/{setup}",
            repo = repo,
            branch = branch,
            setup = setup
        );
        info!("Fetching {}", url);
        match client
            .get(url)
            .send()
            .instrument(debug_span!("Github request"))
            .await
        {
            Ok(response) => {
                let body = response
                    .text()
                    .instrument(debug_span!("Github: Fetch content"))
                    .await?;
                let is_npe2 = ["napari.yaml", "napari.yml"]
                    .into_iter()
                    .any(|tgt| body.find(tgt).is_some());
                any = true;
                if is_npe2 {
                    info!("DETECTED NPE2 for {}", repo);
                    return Ok((true, true));
                }
            }
            Err(_) => {}
        }
    }
    info!("No npe2 for {}", repo);
    Ok((any, false))
}

#[instrument]
async fn run() -> Result<()> {
    let resp = reqwest::get("https://api.napari-hub.org/plugins")
        .instrument(debug_span!("napari-hub request plugin list"))
        .await?
        .json::<HashMap<String, String>>()
        .instrument(debug_span!("napari-hub plugin listing json content"))
        .await?;

    let date_cutoff =
        // DateTime::parse_from_str("2022-01-17 12:24:00 -08:00", "%Y-%m-%d %H:%M:%S %z")?;
        DateTime::parse_from_str("2022-02-01 00:00:00 -08:00", "%Y-%m-%d %H:%M:%S %z")?;

    let client = Client::new();
    let bodies = stream::iter(resp.keys())
        .map(|plugin| {
            let client = &client;
            async move {
                client
                    .get(format!("https://api.napari-hub.org/plugins/{}", plugin))
                    .send()
                    .instrument(debug_span!("GET napari-hub Header"))
                    .await?
                    .text()
                    .instrument(debug_span!("GET napari-hub Content"))
                    .await
            }
        })
        .buffer_unordered(CONCURRENT_REQUESTS);

    let stats = bodies
        .filter_map(|res| {
            ready(if let Ok(body) = res {
                let res: serde_json::Value =
                    serde_json::from_str(&body).expect("Expected a json body.");
                let release_date =
                    DateTime::parse_from_rfc3339(res["release_date"].as_str().unwrap())
                        .expect("Could not parse as rfc3339 datetime string");
                if release_date > date_cutoff {
                    Some(res)
                } else {
                    None
                }
            } else {
                None
            })
        })
        .fold(Stats::default(), |acc, json| {
            let repo = json["code_repository"]
                .as_str()
                .expect("missing 'code_repository' field")
                .trim_start_matches("https://github.com/")
                .to_owned();
            let branches = ["main", "master"];
            let client = &client;
            {
                let repo = repo.clone();
                async move {
                    let mut stats = acc;
                    stats.total += 1;
                    for branch in branches {
                        if let Ok((any, is_npe2)) = analyze_plugin(client, &repo, branch).await {
                            if any && !is_npe2 {
                                stats.stragglers.push(repo.clone());
                            }
                            if any || is_npe2 {
                                stats.success += any as usize;
                                stats.npe2 += is_npe2 as usize;
                                break;
                            }
                        }
                    }
                    stats
                }
            }
            .instrument(info_span!("Analyze", repo = repo.as_str()))
        })
        .instrument(info_span!("Aggregate stats"))
        .await;

    let mut stats = stats;
    stats.stragglers.sort();

    dbg!((&stats, stats.npe2 as f32 / (stats.success as f32)));
    Ok(())
}

fn main() -> Result<()> {
    let (chrome_layer, _guard) = ChromeLayerBuilder::new().include_args(true).build();
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(chrome_layer)
        .with(EnvFilter::from_default_env())
        .init();

    let rt = Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(run())?;

    Ok(())
}
