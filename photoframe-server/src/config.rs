use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::RwLock;
use toml_edit::{DocumentMut, Item, value};

#[cfg(feature = "embed_ui")]
use rust_embed::RustEmbed;

#[cfg(feature = "embed_ui")]
#[derive(RustEmbed)]
#[folder = "../"]
#[include = "photoframe.example.toml"]
struct ConfigAssets;

/// Default on-disk config filename
pub const DEFAULT_CONFIG_PATH: &str = "photoframe.toml";

/// Strongly typed representation of the configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub env: Option<String>,
    pub server: Option<Server>,
    pub logging: Option<Logging>,
    /// Optional global limits for original (base) image dimensions. Images larger than these
    /// limits will be downscaled (aspect preserved) before being cached/saved as base images.
    pub image_limits: Option<ImageLimits>,
    #[serde(default)]
    pub photoframes: std::collections::HashMap<String, PhotoFrame>,
    #[serde(default)]
    pub sources: std::collections::HashMap<String, Source>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Server {
    pub bind_address: Option<String>,
    pub public_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Logging {
    pub filter: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ImageLimits {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Landscape,
    Portrait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScalingMode {
    #[default]
    Contain,
    Cover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// Encode and upload as PNG (default).
    #[default]
    Png,
    /// Raw packed 4 bits-per-pixel (two pixels per byte), left-to-right, top-to-bottom.
    Packed4bpp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UploadTransport {
    /// Send as raw bytes in the HTTP body with appropriate content-type (default).
    #[default]
    Raw,
    /// Send as multipart/form-data with a single file part named "file".
    Multipart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrderKind {
    #[default]
    Random,
    Sequential,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PhotoFrame {
    pub orientation: Option<Orientation>,
    pub scaling: Option<ScalingMode>,
    pub upload_endpoint: Option<String>,
    pub panel_width: Option<u32>,
    pub panel_height: Option<u32>,
    /// Flip the image 180° relative to the default orientation handling.
    /// When true, the final rendered image is turned upside down.
    pub flip: Option<bool>,
    /// Output encoding used when pushing to device.
    pub output_format: Option<OutputFormat>,
    /// HTTP body transport used for device upload.
    pub upload_transport: Option<UploadTransport>,
    #[serde(default)]
    pub source_ids: Vec<String>,
    pub update_cron: Option<croner::Cron>,
    pub dithering: Option<String>,
    #[serde(default)]
    pub supported_colors: Vec<String>,
    pub overscan: Option<Overscan>,
    pub adjustments: Option<Adjustments>,
    /// Packed 4bpp devices vary in nibble order. When true, pack low-nibble first (left pixel).
    /// Default is false, meaning high-nibble first.
    pub swap_nibbles: Option<bool>,
    /// Reverse row order (bottom-to-top) when packing raw streams.
    pub reverse_rows: Option<bool>,
    /// Reverse column order (right-to-left) when packing raw streams.
    pub reverse_cols: Option<bool>,
    #[serde(default)]
    pub dummy: bool,
    #[serde(default)]
    pub paused: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Overscan {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Adjustments {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub sharpness: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum Source {
    #[serde(rename = "filesystem")]
    Filesystem {
        filesystem: Option<FilesystemSource>,
    },
    #[serde(rename = "immich")]
    Immich { immich: Option<ImmichSource> },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FilesystemSource {
    pub glob: Option<String>,
    pub order: Option<OrderKind>,
}
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ImmichSource {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub order: Option<OrderKind>,
    /// Arbitrary search filters passed directly to Immich `searchAssets` endpoint body.
    /// This allows specifying albumIds, personIds, etc. Always merged with type=IMAGE.
    pub filters: Option<serde_json::Value>,
}

/// Internal manager state kept behind an `Arc<RwLock<_>>`.
#[derive(Debug)]
pub struct ConfigManager {
    path: PathBuf,
    doc: DocumentMut,
}

pub type SharedConfig = Arc<RwLock<ConfigManager>>;

impl ConfigManager {
    /// Load existing config file. If the file does not exist, creates it from the embedded example.
    pub async fn load(path: Option<PathBuf>) -> Result<SharedConfig> {
        let path = path.unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));

        // Check if config file exists, if not create it from embedded example
        if !path.exists() {
            #[cfg(feature = "embed_ui")]
            {
                if let Some(example_file) = ConfigAssets::get("photoframe.example.toml") {
                    let example_content = std::str::from_utf8(&example_file.data)
                        .with_context(|| "embedded example config is not valid UTF-8")?;

                    fs::write(&path, example_content)
                        .await
                        .with_context(|| format!("writing example config to {}", path.display()))?;

                    println!("📝 Created default config file: {}", path.display());
                    println!(
                        "   Please edit this file to configure your photo frames and sources."
                    );
                    println!("   The server will continue running with the default configuration.");
                } else {
                    bail!(
                        "config file {} not found and embedded example is not available",
                        path.display()
                    );
                }
            }
            #[cfg(not(feature = "embed_ui"))]
            {
                bail!("config file {} not found", path.display());
            }
        }

        let text = fs::read_to_string(&path)
            .await
            .with_context(|| format!("reading config file {}", path.display()))?;
        let doc = text.parse::<DocumentMut>()?;
        Ok(Arc::new(RwLock::new(Self { path, doc })))
    }

    /// Convert current document to strongly typed struct.
    pub async fn to_struct(cfg: &SharedConfig) -> Result<Config> {
        let guard = cfg.read().await;
        let typed: Config = toml_edit::de::from_document(guard.doc.clone())?;
        Ok(typed)
    }

    /// Set dithering algorithm string for a given photoframe id.
    pub async fn set_frame_dithering(
        cfg: &SharedConfig,
        frame_id: &str,
        dithering: &str,
    ) -> Result<()> {
        let mut guard = cfg.write().await;
        let frames = guard.doc["photoframes"]
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("photoframes table missing"))?;
        let frame = frames
            .get_mut(frame_id)
            .ok_or_else(|| anyhow::anyhow!("photoframe '{}' not found", frame_id))?;
        if let Item::Table(tbl) = frame {
            tbl["dithering"] = value(dithering);
        } else {
            bail!("photoframe '{}' is not a table", frame_id);
        }
        Ok(())
    }

    /// Update adjustment parameters for a frame. Only provided values are updated.
    pub async fn update_frame_adjustments(
        cfg: &SharedConfig,
        frame_id: &str,
        brightness: Option<f32>,
        contrast: Option<f32>,
        saturation: Option<f32>,
        sharpness: Option<f32>,
    ) -> Result<()> {
        let mut guard = cfg.write().await;
        let frames = guard.doc["photoframes"]
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("photoframes table missing"))?;
        let frame = frames
            .get_mut(frame_id)
            .ok_or_else(|| anyhow::anyhow!("photoframe '{}' not found", frame_id))?;
        if let Item::Table(tbl) = frame {
            let adjustments = tbl["adjustments"].or_insert(Item::Table(toml_edit::Table::new()));
            if let Item::Table(adj_tbl) = adjustments {
                if let Some(v) = brightness {
                    adj_tbl["brightness"] = value(v as f64);
                }
                if let Some(v) = contrast {
                    adj_tbl["contrast"] = value(v as f64);
                }
                if let Some(v) = saturation {
                    adj_tbl["saturation"] = value(v as f64);
                }
                if let Some(v) = sharpness {
                    adj_tbl["sharpness"] = value(v as f64);
                }
            }
        } else {
            bail!("photoframe '{}' is not a table", frame_id);
        }
        Ok(())
    }

    /// Update overscan padding values for a frame. Only provided values are changed.
    pub async fn update_frame_overscan(
        cfg: &SharedConfig,
        frame_id: &str,
        left: Option<i32>,
        right: Option<i32>,
        top: Option<i32>,
        bottom: Option<i32>,
    ) -> Result<()> {
        let mut guard = cfg.write().await;
        let frames = guard.doc["photoframes"]
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("photoframes table missing"))?;
        let frame = frames
            .get_mut(frame_id)
            .ok_or_else(|| anyhow::anyhow!("photoframe '{}' not found", frame_id))?;
        if let Item::Table(tbl) = frame {
            let osc = tbl["overscan"].or_insert(Item::Table(toml_edit::Table::new()));
            if let Item::Table(otbl) = osc {
                if let Some(v) = left {
                    otbl["left"] = value(v as i64);
                }
                if let Some(v) = right {
                    otbl["right"] = value(v as i64);
                }
                if let Some(v) = top {
                    otbl["top"] = value(v as i64);
                }
                if let Some(v) = bottom {
                    otbl["bottom"] = value(v as i64);
                }
            }
        } else {
            bail!("photoframe '{}' is not a table", frame_id);
        }
        Ok(())
    }

    /// Set paused flag for a frame.
    pub async fn set_frame_paused(cfg: &SharedConfig, frame_id: &str, paused: bool) -> Result<()> {
        let mut guard = cfg.write().await;
        let frames = guard.doc["photoframes"]
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("photoframes table missing"))?;
        let frame = frames
            .get_mut(frame_id)
            .ok_or_else(|| anyhow::anyhow!("photoframe '{}' not found", frame_id))?;
        if let Item::Table(tbl) = frame {
            tbl["paused"] = value(paused);
            Ok(())
        } else {
            bail!("photoframe '{}' is not a table", frame_id);
        }
    }

    /// Set dummy flag for a frame.
    pub async fn set_frame_dummy(cfg: &SharedConfig, frame_id: &str, dummy: bool) -> Result<()> {
        let mut guard = cfg.write().await;
        let frames = guard.doc["photoframes"]
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("photoframes table missing"))?;
        let frame = frames
            .get_mut(frame_id)
            .ok_or_else(|| anyhow::anyhow!("photoframe '{}' not found", frame_id))?;
        if let Item::Table(tbl) = frame {
            tbl["dummy"] = value(dummy);
            Ok(())
        } else {
            bail!("photoframe '{}' is not a table", frame_id);
        }
    }

    /// Set flip flag for a frame.
    pub async fn set_frame_flip(cfg: &SharedConfig, frame_id: &str, flip: bool) -> Result<()> {
        let mut guard = cfg.write().await;
        let frames = guard.doc["photoframes"]
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("photoframes table missing"))?;
        let frame = frames
            .get_mut(frame_id)
            .ok_or_else(|| anyhow::anyhow!("photoframe '{}' not found", frame_id))?;
        if let Item::Table(tbl) = frame {
            tbl["flip"] = value(flip);
            Ok(())
        } else {
            bail!("photoframe '{}' is not a table", frame_id);
        }
    }

    /// Atomic write of current document to disk (best-effort durability via rename).
    pub async fn save(cfg: &SharedConfig) -> Result<()> {
        let (path, contents) = {
            let guard = cfg.read().await;
            (guard.path.clone(), guard.doc.to_string())
        };
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, contents)
            .await
            .with_context(|| format!("writing tmp config {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .await
            .with_context(|| format!("renaming tmp config to {}", path.display()))?;
        Ok(())
    }

    /// Persist API credentials for an existing Immich source.
    pub async fn set_immich_credentials(
        cfg: &SharedConfig,
        source_id: &str,
        base_url: &str,
        api_key: &str,
    ) -> Result<()> {
        let mut guard = cfg.write().await;
        let sources_tbl = guard.doc["sources"]
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("sources table missing"))?;
        let src = sources_tbl
            .get_mut(source_id)
            .ok_or_else(|| anyhow::anyhow!("source '{}' not found", source_id))?;
        if let Item::Table(tbl) = src {
            if tbl.get("kind").is_none() {
                tbl["kind"] = value("immich");
            }
            let im = tbl["immich"].or_insert(Item::Table(toml_edit::Table::new()));
            if let Item::Table(imt) = im {
                imt["base_url"] = value(base_url);
                imt["api_key"] = value(api_key);
            }
        }
        Ok(())
    }

    /// Update Immich source filters JSON object (replaces previous value).
    pub async fn set_immich_filters(
        cfg: &SharedConfig,
        source_id: &str,
        filters: &serde_json::Value,
    ) -> Result<()> {
        if !filters.is_object() {
            bail!("filters must be a JSON object");
        }
        let mut guard = cfg.write().await;
        let sources_tbl = guard.doc["sources"]
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("sources table missing"))?;
        let src = sources_tbl
            .get_mut(source_id)
            .ok_or_else(|| anyhow::anyhow!("source '{}' not found", source_id))?;
        if let Item::Table(tbl) = src {
            if tbl.get("kind").is_none() {
                tbl["kind"] = value("immich");
            }
            let im = tbl["immich"].or_insert(Item::Table(toml_edit::Table::new()));
            if let Item::Table(imt) = im {
                // store raw JSON string then parse back into toml via serde roundtrip
                // Convert serde_json::Value -> toml_edit::Item using string intermediate.
                let serialized = serde_json::to_string(filters)?;
                // Represent as inline table via toml parsing
                let parsed: toml_edit::Value = toml_edit::Value::from(serialized);
                imt["filters"] = Item::Value(parsed);
            }
        }
        Ok(())
    }
}
