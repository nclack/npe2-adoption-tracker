use std::collections::HashMap;

use anyhow::{anyhow, Result};
use chrono::DateTime;

#[tokio::main]
async fn main() -> Result<()> {
    let resp = reqwest::get("https://api.napari-hub.org/plugins")
        .await?
        .json::<HashMap<String, String>>()
        .await?;

    let date_cutoff =
        // DateTime::parse_from_str("2022-01-17 12:24:00 -08:00", "%Y-%m-%d %H:%M:%S %z")?;
        DateTime::parse_from_str("2022-02-01 00:00:00 -08:00", "%Y-%m-%d %H:%M:%S %z")?;
    let mut hits = 0;
    let mut npe2s = 0;
    for plugin in resp.keys() {
        let body = reqwest::get(format!("https://api.napari-hub.org/plugins/{}", plugin))
            .await?
            .text()
            .await?;
        let res: serde_json::Value = serde_json::from_str(&body)?;
        let release_date = DateTime::parse_from_rfc3339(res["release_date"].as_str().unwrap())?;
        println!(
            "{:?}  {:?} {:?}",
            res["code_repository"],
            release_date,
            release_date > date_cutoff
        );

        if release_date > date_cutoff {
            let repo = res["code_repository"]
                .as_str()
                .ok_or(anyhow!("missing 'code_repository' field"))?
                .trim_start_matches("https://github.com/");
            let branches = ["main", "master"];
            let setups = ["setup.cfg", "setup.py"];
            let mut any = false;
            let mut is_npe2 = false;
            'combos: for branch in branches {
                for setup in setups {
                    let url = format!(
                        "https://raw.githubusercontent.com/{repo}/{branch}/{setup}",
                        repo = repo,
                        branch = branch,
                        setup = setup
                    );
                    match reqwest::get(url).await {
                        Ok(response) => {
                            let body = response.text().await?;
                            is_npe2 |= ["napari.yaml", "napari.yml"]
                                .into_iter()
                                .any(|tgt| body.find(tgt).is_some());
                            any = true;
                            if is_npe2 {
                                break 'combos;
                            }
                        }
                        Err(_) => todo!(),
                    }
                }
                if any {
                    break 'combos;
                }
            }
            dbg!((plugin, any, is_npe2));
            if any {
                hits += 1;
                npe2s += if is_npe2 { 1 } else { 0 };
            }
        }
    }
    dbg!((hits,npe2s,npe2s as f32/(hits as f32)));

    Ok(())
}
