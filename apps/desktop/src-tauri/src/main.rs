//! HTTP Client Pro — Tauri 桌面端入口。
//!
//! 通过 Tauri IPC commands 直接调用 http-core，无需单独运行 http-web 后端。
//! 前端通过 `@tauri-apps/api/core` 的 `invoke()` 调用这些命令。

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use http_core::dispatch::Dispatcher;
use http_core::env::Environment;
use http_core::model::RequestTarget;
use http_core::parser;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// 健康检查 — 前端 `TauriAdapter.health()` 调用。
#[tauri::command]
fn ping() -> String {
    "ok".to_string()
}

/// 执行 `.http` 源码中的第一个请求 — 前端 `TauriAdapter.execute()` 调用。
///
/// 返回与 http-web `POST /execute` 完全相同的 JSON 形状：
/// `{ status, headers, body, elapsed_ms, url }`
#[tauri::command]
async fn execute_http(source: String) -> Result<Value, String> {
    let file = parser::parse_file(&source).map_err(|e| e.to_string())?;
    let req = file
        .requests
        .into_iter()
        .next()
        .ok_or_else(|| "source contains no request".to_string())?;
    let env = Environment::new();
    let dispatcher = Dispatcher::new();
    let res = dispatcher
        .send(&req, &env, None)
        .await
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "status": res.status,
        "headers": res.headers.iter()
            .map(|(n, v)| (n.clone(), Value::from(v.clone())))
            .collect::<serde_json::Map<String, Value>>(),
        "body": String::from_utf8_lossy(&res.body).into_owned(),
        "elapsed_ms": res.elapsed.as_millis() as u64,
        "url": res.url,
    }))
}

/// 将 RequestTarget 转换为可读字符串。
fn target_to_string(target: &RequestTarget) -> String {
    match target {
        RequestTarget::Origin { path, query, fragment } => {
            let mut s = path.clone();
            if let Some(q) = query {
                s.push('?');
                s.push_str(q);
            }
            if let Some(f) = fragment {
                s.push('#');
                s.push_str(f);
            }
            s
        }
        RequestTarget::Absolute { scheme, authority, path, query, fragment } => {
            let mut s = String::new();
            if let Some(sc) = scheme {
                s.push_str(sc);
                s.push_str("://");
            }
            s.push_str(authority);
            if let Some(p) = path {
                s.push_str(p);
            }
            if let Some(q) = query {
                s.push('?');
                s.push_str(q);
            }
            if let Some(f) = fragment {
                s.push('#');
                s.push_str(f);
            }
            s
        }
        RequestTarget::Asterisk => "*".to_string(),
    }
}

/// 列出 `.http` 源码中的所有请求 — 前端 `listRequests` 调用。
#[tauri::command]
fn list_requests(source: String) -> Result<Value, String> {
    let file = parser::parse_file(&source).map_err(|e| e.to_string())?;
    let requests: Vec<Value> = file
        .requests
        .iter()
        .enumerate()
        .map(|(i, r)| {
            json!({
                "index": i,
                "name": r.name,
                "method": r.line.method.to_string(),
                "target": target_to_string(&r.line.target),
            })
        })
        .collect();
    Ok(json!({ "requests": requests }))
}

/// 列出目录下的文件和子目录（一层）
#[tauri::command]
fn list_dir(path: String) -> Result<Value, String> {
    let entries = fs::read_dir(&path).map_err(|e| format!("Failed to read dir: {e}"))?;
    let mut items: Vec<Value> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // 跳过隐藏文件
        if name.starts_with('.') {
            continue;
        }
        let is_dir = entry
            .file_type()
            .map(|t| t.is_dir())
            .unwrap_or(false);
        let full_path = entry.path().to_string_lossy().into_owned();
        items.push(json!({
            "name": name,
            "path": full_path,
            "isDir": is_dir,
        }));
    }
    // 目录在前，按名称排序
    items.sort_by(|a, b| {
        let a_dir = a["isDir"].as_bool().unwrap_or(false);
        let b_dir = b["isDir"].as_bool().unwrap_or(false);
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_name = a["name"].as_str().unwrap_or("");
                let b_name = b["name"].as_str().unwrap_or("");
                a_name.cmp(b_name)
            }
        }
    });
    Ok(json!({ "items": items }))
}

/// 读取文件内容
#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {e}"))
}

/// 写入文件内容（如不存在则创建）
#[tauri::command]
fn write_file(path: String, content: String) -> Result<(), String> {
    fs::write(&path, &content).map_err(|e| format!("Failed to write file: {e}"))
}

/// 新建文件
#[tauri::command]
fn create_file(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if p.exists() {
        return Err(format!("File already exists: {path}"));
    }
    fs::write(&path, "").map_err(|e| format!("Failed to create file: {e}"))
}

/// 新建目录
#[tauri::command]
fn create_dir(path: String) -> Result<(), String> {
    fs::create_dir(&path).map_err(|e| format!("Failed to create dir: {e}"))
}

/// 重命名文件或目录
#[tauri::command]
fn rename_path(old_path: String, new_path: String) -> Result<(), String> {
    fs::rename(&old_path, &new_path).map_err(|e| format!("Failed to rename: {e}"))
}

/// 删除文件
#[tauri::command]
fn delete_file(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if p.is_dir() {
        fs::remove_dir_all(&path).map_err(|e| format!("Failed to delete dir: {e}"))
    } else {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete file: {e}"))
    }
}

/// 获取用户主目录下的默认工作空间路径
#[tauri::command]
fn get_default_workspace() -> Result<String, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "Cannot determine home directory".to_string())?;
    let ws = PathBuf::from(home).join(".http-client-pro");
    if !ws.exists() {
        fs::create_dir_all(&ws).map_err(|e| format!("Failed to create workspace: {e}"))?;
    }
    Ok(ws.to_string_lossy().into_owned())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            ping,
            execute_http,
            list_requests,
            list_dir,
            read_file,
            write_file,
            create_file,
            create_dir,
            rename_path,
            delete_file,
            get_default_workspace
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
