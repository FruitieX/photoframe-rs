//! Fast end-to-end smoke test: boots the real HTTP router against a throwaway
//! config (local folder source, dummy frame) and checks the key API routes.
//! No network access and no device uploads.

use std::path::PathBuf;
use std::sync::Arc;

use photoframe_server::{config, http, scheduler};
use serde_json::{Value, json};

fn test_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("photoframe-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_config(dir: &std::path::Path) -> PathBuf {
    let photos = dir.join("photos");
    std::fs::create_dir_all(&photos).unwrap();
    // Landscape, to match the frame's orientation.
    let asset = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.github/smoke/landscape.png"
    );
    std::fs::copy(asset, photos.join("landscape.png")).unwrap();

    let path = dir.join("photoframe.toml");
    std::fs::write(
        &path,
        format!(
            r##"
[photoframes.test]
dummy = true
upload_endpoint = "http://127.0.0.1:9/upload"
orientation = "landscape"
panel_width = 800
panel_height = 480
scaling = "cover"
output_format = "png"
dithering = "floyd_steinberg"
source_ids = ["local"]
update_cron = "0 0 7-23 * * *"
supported_colors = ["#000000", "#ffffff", "#ff0000"]

[sources.local]
kind = "filesystem"

[sources.local.filesystem]
glob = "{}/*.png"
order = "random"
"##,
            photos.display()
        ),
    )
    .unwrap();
    path
}

#[tokio::test]
async fn server_answers_health_config_and_frame_routes() {
    let dir = test_dir();
    let cfg_path = write_config(&dir);
    // Frame updates write their images to the working directory.
    std::env::set_current_dir(&dir).unwrap();

    let cfg = config::ConfigManager::load(Some(cfg_path.clone()))
        .await
        .unwrap();
    let sched = Arc::new(scheduler::FrameScheduler::new(cfg.clone()).await.unwrap());
    sched.populate().await.unwrap();
    let app = http::router(http::AppState {
        cfg,
        scheduler: sched,
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = reqwest::Client::new();

    for route in ["health", "live"] {
        let res = client.get(format!("{base}/{route}")).send().await.unwrap();
        assert!(res.status().is_success(), "/api/{route}: {}", res.status());
    }

    let config: Value = client
        .get(format!("{base}/config"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(config["photoframes"]["test"]["panel_width"], 800);
    assert_eq!(config["sources"]["local"]["kind"], "filesystem");

    let palette: Value = client
        .get(format!("{base}/frames/test/palette"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(palette["palette"].as_array().unwrap().len(), 3);
    assert_eq!(palette["palette"][2]["rgb"], json!([255, 0, 0]));

    // Saving a setting rewrites the TOML file and is reflected in the config.
    client
        .patch(format!("{base}/frames/test"))
        .json(&json!({ "dithering": "atkinson" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let config: Value = client
        .get(format!("{base}/config"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(config["photoframes"]["test"]["dithering"], "atkinson");
    let saved = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(saved.contains(r#"dithering = "atkinson""#), "{saved}");
    assert!(
        saved.contains(r#"update_cron = "0 0 7-23 * * *""#),
        "{saved}"
    );

    // Triggering an update of the dummy frame records a successful update.
    client
        .post(format!("{base}/frames/test/trigger"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let res = client
        .get(base.replace("/api", "/metrics"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert!(
        res.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/plain")
    );
    let metrics = res.text().await.unwrap();
    for line in [
        r#"photoframe_update_attempts_total{frame="test",result="success"} 1"#,
        r#"photoframe_update_attempts_total{frame="test",result="push_failed"} 0"#,
        r#"photoframe_update_consecutive_failures{frame="test"} 0"#,
        r#"photoframe_source_errors_total{kind="search",source="local"} 0"#,
    ] {
        assert!(metrics.contains(line), "missing {line} in:\n{metrics}");
    }
    assert!(metrics.contains("photoframe_last_update_success_timestamp_seconds{frame=\"test\"}"));

    let missing = client
        .get(format!("{base}/frames/nope/palette"))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);

    std::fs::remove_dir_all(&dir).ok();
}
