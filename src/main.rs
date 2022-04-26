use std::collections::HashMap;

use anyhow::{Result};
use chrono::DateTime;
use futures::{
    future::ready,
    stream::{self, StreamExt},
};
use reqwest::Client;

const CONCURRENT_REQUESTS: usize = 2;

#[derive(Debug, Default)]
struct Stats {
    total: usize,
    success: usize,
    npe2: usize,
}

// async fn analyze_plugin(repo: &str, branch: &str) -> Result<(bool, bool)> {
//     let setups = ["setup.cfg", "setup.py"];
//     let mut any=false;
//     for setup in setups {
//         let url = format!(
//             "https://raw.githubusercontent.com/{repo}/{branch}/{setup}",
//             repo = repo,
//             branch = branch,
//             setup = setup
//         );
//         match reqwest::get(url).await {
//             Ok(response) => {
//                 let body = response.text().await?;
//                 let is_npe2 = ["napari.yaml", "napari.yml"]
//                     .into_iter()
//                     .any(|tgt| body.find(tgt).is_some());
//                 any = true;
//                 if is_npe2 {
//                     return Ok((true,true))
//                 }
//             }
//             Err(_) => {}
//         }
//     }
//     Ok((any,false))
// }

async fn analyze_plugin2(client: &Client, repo: &str, branch: &str) -> Result<(bool, bool)> {
    let setups = ["setup.cfg", "setup.py"];
    let mut any=false;
    for setup in setups {
        let url = format!(
            "https://raw.githubusercontent.com/{repo}/{branch}/{setup}",
            repo = repo,
            branch = branch,
            setup = setup
        );
        match client.get(url).send().await {
            Ok(response) => {
                let body = response.text().await?;
                let is_npe2 = ["napari.yaml", "napari.yml"]
                    .into_iter()
                    .any(|tgt| body.find(tgt).is_some());
                any = true;
                if is_npe2 {
                    return Ok((true,true))
                }
            }
            Err(_) => {}
        }
    }
    Ok((any,false))
}

#[tokio::main]
async fn main() -> Result<()> {
    let resp = reqwest::get("https://api.napari-hub.org/plugins")
        .await?
        .json::<HashMap<String, String>>()
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
                    .await?
                    .text()
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
            let client=&client;
            async move {
                let mut stats=acc;
                stats.total+=1;
                for branch in branches {
                    if let Ok((any,is_npe2))=analyze_plugin2(client,&repo,branch).await {
                        if any || is_npe2 {
                            stats.success+=any as usize;
                            stats.npe2+=is_npe2 as usize;
                            break;
                        }
                    }
                }
                stats
            }
        })
        .await;

    dbg!((&stats,stats.npe2 as f32/(stats.success as f32)));
    Ok(())
}

//     for plugin in resp.keys() {
//         let body = reqwest::get(format!("https://api.napari-hub.org/plugins/{}", plugin))
//             .await?
//             .text()
//             .await?;
//         let res: serde_json::Value = serde_json::from_str(&body)?;
//         let release_date = DateTime::parse_from_rfc3339(res["release_date"].as_str().unwrap())?;
//         println!(
//             "{:?}  {:?} {:?}",
//             res["code_repository"],
//             release_date,
//             release_date > date_cutoff
//         );

//         if release_date > date_cutoff {
//             let repo = res["code_repository"]
//                 .as_str()
//                 .ok_or(anyhow!("missing 'code_repository' field"))?
//                 .trim_start_matches("https://github.com/");
//             let branches = ["main", "master"];
//             let mut any = false;
//             let mut is_npe2 = false;
//             for branch in branches {
//                 let (any_,is_npe2_)=analyze_plugin(repo, branch).await?;
//                 any|=any_;
//                 is_npe2|=is_npe2_;
//                 if is_npe2 || any {
//                     break;
//                 }
//             }
//             dbg!((plugin, any, is_npe2));
//             if any {
//                 hits += 1;
//                 npe2s += if is_npe2 { 1 } else { 0 };
//             }
//         }
//     }
//     dbg!((hits, npe2s, npe2s as f32 / (hits as f32)));

//     Ok(())
// }
