// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ole;
mod settings;

use crate::ole::{initialize_com, DeferQuit, DeferUninitializeCOM, IDispatchWrapper};
use crate::settings::Settings;
use anyhow::Context;
use log::info;
use serde::Serialize;
use std::error::Error;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::{env, fs};
use tempfile::NamedTempFile;

#[tauri::command]
fn generate(
    path: String,
    submit_type: Vec<String>,
    event_mode: Vec<String>,
    handle: tauri::AppHandle,
) -> Result<String, String> {
    fs::File::open(&path).map_err(|err| err.to_string())?;
    let settings = Settings::new(
        handle
            .path_resolver()
            .app_config_dir()
            .expect("failed to get app config dir")
            .join("config.toml")
            .as_path(),
    )
    .map_err(|err| err.to_string())?;

    let md =
        convert2md(&path, &submit_type, &event_mode, &settings).map_err(|err| err.to_string())?;
    let docx = convert2docx(&md, &settings, &handle).map_err(|err| err.to_string())?;
    let _pdf = convert2pdf(&docx).map_err(|err| err.to_string())?;

    // Return nothing on success
    Ok("Ok!".to_string())
}

#[tauri::command]
async fn open_directory(path: String) -> Result<(), String> {
    let path = Path::new(&path);
    let dir = path.parent().unwrap();

    #[cfg(target_family = "unix")]
    {
        Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|_| "Failed to open directory with xdg-open".to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg(dir)
            .spawn()
            .map_err(|_| "Failed to open directory with explorer.exe".to_string())?;
    }

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ConvertContext {
    enabled_template: bool,
    enabled_text: bool,
    enabled_poll_mode: bool,
    enabled_push_mode: bool,
}

fn convert2md(
    path: &String,
    submit_type: &Vec<String>,
    event_mode: &Vec<String>,
    settings: &Settings,
) -> Result<String, Box<dyn Error>> {
    info!("convert2md: processing...");

    let ctx = ConvertContext {
        enabled_template: submit_type.contains(&String::from("template")),
        enabled_text: submit_type.contains(&String::from("text")),
        enabled_poll_mode: event_mode.contains(&String::from("poll")),
        enabled_push_mode: event_mode.contains(&String::from("push")),
    };

    let src = path;
    let dst = Path::new(path)
        .with_extension("md")
        .as_path()
        .display()
        .to_string();

    info!("convert2md: {src} => {dst}");

    let gotmpl_path = settings
        .gotmpl
        .binary
        .clone()
        .unwrap_or(String::from("gotmpl"));

    info!("gotmpl_path: {gotmpl_path}");

    let data = serde_json::to_string(&ctx)?;
    let mut tmp = NamedTempFile::with_prefix("gendoc_")?;
    tmp.write_all(data.as_bytes())?;

    // let status = tauri::api::process::Command::new_sidecar("gotmpl")
    //     .expect("failed to create `gotmpl` binary command")
    //     .args([
    //         String::from("-i"),
    //         tmp.path().to_string_lossy().into_owned(),
    //         String::from("-o"),
    //         dst.to_string(),
    //         src.to_string(),
    //     ])
    //     .status()
    //     .expect("failed to execute `gotmpl` binary command");

    let status = Command::new(&gotmpl_path)
        .arg("-i")
        .arg(tmp.path().to_string_lossy().into_owned())
        .arg("-o")
        .arg(&dst)
        .arg(src)
        .status()
        .expect("failed to execute `gotmpl` binary command");

    info!("convert2md: process finished with: {status}");

    if !status.success() {
        Err(Box::from(format!(
            "Template => Markdown 转换失败: {status}"
        )))
    } else {
        Ok(dst)
    }
}

fn convert2docx(
    path: &String,
    settings: &Settings,
    handle: &tauri::AppHandle,
) -> Result<String, Box<dyn Error>> {
    info!("convert2docx: processing...");

    let src = path;
    let dst = Path::new(path)
        .with_extension("docx")
        .into_os_string()
        .into_string()
        .unwrap();

    info!("convert2docx: {src} => {dst}");

    let pandoc_path = settings
        .pandoc
        .binary
        .clone()
        .unwrap_or(String::from("pandoc"));

    let reference_doc_path = settings.pandoc.reference_doc.clone().unwrap_or_else(|| {
        handle
            .path_resolver()
            .resolve_resource("resources/pandoc/custom-reference.docx")
            .map(|p| p.to_string_lossy().into_owned())
            .expect("failed to resolve pandoc reference doc")
    });

    let reference_doc_path = reference_doc_path.trim_start_matches("\\\\?\\").to_owned();

    info!("pandoc_path: {pandoc_path}");
    info!("reference_doc_path: {reference_doc_path}");

    // let status = tauri::api::process::Command::new_sidecar("pandoc")
    //     .expect("failed to create `pandoc` binary command")
    //     .args([
    //         String::from("--reference-doc"),
    //         reference_doc_path,
    //         String::from("-o"),
    //         dst.to_string(),
    //         src.to_string(),
    //     ])
    //     .status()
    //     .expect("failed to execute `pandoc` binary command");

    let status = Command::new(&pandoc_path)
        .arg("--reference-doc")
        .arg(&reference_doc_path)
        .arg("-o")
        .arg(&dst)
        .arg(src)
        .status()
        .expect("failed to execute `pandoc` binary command");

    info!("convert2docx: process finished with: {status}");

    if !status.success() {
        Err(Box::from(format!("Markdown => Docx 转换失败: {status}")))
    } else {
        Ok(dst)
    }
}

fn office2pdf(src: &String, dst: &String) -> Result<i32, Box<dyn Error>> {
    let word = IDispatchWrapper::new(&String::from("Word.Application"))?;
    let _word = DeferQuit(&word);

    word.put("Visible", vec![false.into()])
        .with_context(|| "Visible false")?;

    let result = word.get("Documents").with_context(|| "get Documents")?;

    let documents = result.idispatch().with_context(|| "idispatch Documents")?;

    let result = documents
        .call("Open", vec![src.into()])
        .with_context(|| format!("Failed to open file \"{}\"!", src))?;

    let document = result.idispatch().with_context(|| "idispatch Document")?;

    let result = document
        .call(
            "ExportAsFixedFormat",
            vec![
                dst.into(),
                17.into(),
                false.into(),
                0.into(),
                0.into(),
                0.into(),
                0.into(),
                0.into(),
                false.into(),
                false.into(),
                1.into(),
                true.into(),
                true.into(),
            ],
        )
        .with_context(|| format!("Failed to open file \"{}\"!", src))?;

    document
        .call("Close", vec![])
        .with_context(|| "call Close")?;

    Ok(result.int()?)
}

fn wps2pdf(src: &String, dst: &String) -> Result<i32, Box<dyn Error>> {
    let word = IDispatchWrapper::new(&String::from("KWps.Application"))?;
    let _word = DeferQuit(&word);

    word.put("Visible", vec![false.into()])
        .with_context(|| "Visible false")?;

    let result = word.get("Documents").with_context(|| "get Documents")?;

    let documents = result.idispatch().with_context(|| "idispatch Documents")?;

    let result = documents
        .call("Open", vec![src.into()])
        .with_context(|| format!("Failed to open file \"{}\"!", src))?;

    let document = result.idispatch().with_context(|| "idispatch Document")?;

    let result = document
        .call("ExportAsFixedFormat", vec![dst.into(), 17.into()])
        .with_context(|| format!("Failed to open file \"{}\"!", src))?;

    document
        .call("Close", vec![])
        .with_context(|| "call Close")?;

    Ok(result.int()?)
}

fn convert2pdf(path: &String) -> Result<String, Box<dyn Error>> {
    info!("convert2pdf: processing...");

    let src = path;
    let dst = Path::new(path)
        .with_extension("pdf")
        .into_os_string()
        .into_string()
        .unwrap();

    info!("convert2pdf: {src} => {dst}");

    initialize_com()?;
    let _com = DeferUninitializeCOM;

    info!("Attempt to export PDF using Microsoft Word...");
    let status = office2pdf(src, &dst)
        .or_else(|err| {
            info!("Failed to export PDF using Microsoft Word: {}", err);
            info!("Attempt to export PDF using KingSoft WPS...");
            wps2pdf(src, &dst)
        })
        .or_else(|err| {
            info!("Failed to export PDF using KingSoft WPS: {}", err);
            Err("Failed to export PDF using Microsoft Word and Kingsoft WPS")
        })?;

    info!("convert2docx: process finished with: {status}");

    if status != 0 {
        Err(Box::from(format!("Docx => PDF 转换失败: {status}")))
    } else {
        info!("convert2pdf: process finished");
        Ok(dst)
    }
}

fn setup_logger(app: &tauri::App) {
    env::set_var(
        "GENDOC_LOG_PATH",
        app.path_resolver()
            .app_log_dir()
            .expect("failed to get app log dir")
            .to_string_lossy()
            .into_owned(),
    );
    log4rs::init_file("config/log4rs.yml", Default::default()).unwrap();
}

fn setup_settings(app: &tauri::App) {
    let path = app
        .path_resolver()
        .app_config_dir()
        .expect("failed to get app config dir")
        .join("config.toml");

    Settings::new(&path.as_path())
        .expect("failed to get app config dir")
        .save(&path.as_path())
        .expect("failed to save config");
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            setup_logger(&app);
            setup_settings(&app);

            info!("starting up");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![generate, open_directory])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
