// Copyright 2026 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::Helper;
use crate::auth;
use crate::error::GwsError;
use crate::executor;
use clap::{Arg, ArgMatches, Command};
use serde_json::{json, Value};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

pub struct DriveHelper;

impl Helper for DriveHelper {
    fn inject_commands(
        &self,
        mut cmd: Command,
        _doc: &crate::discovery::RestDescription,
    ) -> Command {
        cmd = cmd.subcommand(
            Command::new("+upload")
                .about("[Helper] Upload a file with automatic metadata")
                .arg(
                    Arg::new("file")
                        .help("Path to file to upload")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::new("parent")
                        .long("parent")
                        .help("Parent folder ID")
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .help("Target filename (defaults to source filename)")
                        .value_name("NAME"),
                )
                .after_help(
                    "\
EXAMPLES:
  gws drive +upload ./report.pdf
  gws drive +upload ./report.pdf --parent FOLDER_ID
  gws drive +upload ./data.csv --name 'Sales Data.csv'

TIPS:
  MIME type is detected automatically.
  Filename is inferred from the local path unless --name is given.",
                ),
        );
        cmd = cmd.subcommand(
            Command::new("+download")
                .about("[Helper] Download a Drive file to a local path")
                .arg(
                    Arg::new("file")
                        .long("file")
                        .help("Drive file ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("output")
                        .long("output")
                        .help("Output file path (defaults to the file's name in Drive)")
                        .value_name("PATH"),
                )
                .arg(
                    Arg::new("mime-type")
                        .long("mime-type")
                        .help(
                            "Export MIME type for Google Workspace native files \
                             (e.g. application/pdf, text/csv, \
                             application/vnd.openxmlformats-officedocument.wordprocessingml.document). \
                             Required for Docs/Sheets/Slides; ignored for binary files.",
                        )
                        .value_name("TYPE"),
                )
                .after_help(
                    "\
EXAMPLES:
  gws drive +download --file FILE_ID
  gws drive +download --file FILE_ID --output report.pdf
  gws drive +download --file FILE_ID --mime-type application/pdf
  gws drive +download --file FILE_ID --mime-type text/csv --output data.csv

TIPS:
  For Google Docs/Sheets/Slides, provide --mime-type to choose the export format.
  For binary files (PDFs, images, etc.), --mime-type is not needed.
  Output path must be relative to the current directory.",
                ),
        );
        cmd
    }

    fn handle<'a>(
        &'a self,
        doc: &'a crate::discovery::RestDescription,
        matches: &'a ArgMatches,
        _sanitize_config: &'a crate::helpers::modelarmor::SanitizeConfig,
    ) -> Pin<Box<dyn Future<Output = Result<bool, GwsError>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(matches) = matches.subcommand_matches("+upload") {
                let file_path = matches.get_one::<String>("file").unwrap();
                let parent_id = matches.get_one::<String>("parent");
                let name_arg = matches.get_one::<String>("name");

                // Determine filename
                let filename = determine_filename(file_path, name_arg.map(|s| s.as_str()))?;

                // Find method: files.create
                let files_res = doc
                    .resources
                    .get("files")
                    .ok_or_else(|| GwsError::Discovery("Resource 'files' not found".to_string()))?;
                let create_method = files_res.methods.get("create").ok_or_else(|| {
                    GwsError::Discovery("Method 'files.create' not found".to_string())
                })?;

                // Build metadata
                let metadata = build_metadata(&filename, parent_id.map(|s| s.as_str()))?;

                let body_str = metadata.to_string();

                let scopes: Vec<&str> = create_method.scopes.iter().map(|s| s.as_str()).collect();
                let (token, auth_method) = match auth::get_token(&scopes).await {
                    Ok(t) => (Some(t), executor::AuthMethod::OAuth),
                    Err(_) if matches.get_flag("dry-run") => (None, executor::AuthMethod::None),
                    Err(e) => return Err(GwsError::Auth(format!("Drive auth failed: {e}"))),
                };

                executor::execute_method(
                    doc,
                    create_method,
                    None,
                    Some(&body_str),
                    token.as_deref(),
                    auth_method,
                    None,
                    Some(executor::UploadSource::File {
                        path: file_path,
                        content_type: None,
                    }),
                    matches.get_flag("dry-run"),
                    &executor::PaginationConfig::default(),
                    None,
                    &crate::helpers::modelarmor::SanitizeMode::Warn,
                    &crate::formatter::OutputFormat::default(),
                    false,
                )
                .await?;

                return Ok(true);
            }

            if let Some(matches) = matches.subcommand_matches("+download") {
                handle_download(doc, matches).await?;
                return Ok(true);
            }

            Ok(false)
        })
    }
}

async fn handle_download(
    doc: &crate::discovery::RestDescription,
    matches: &ArgMatches,
) -> Result<(), GwsError> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let file_id =
        crate::validate::validate_resource_name(matches.get_one::<String>("file").unwrap())?;
    let output_arg = matches.get_one::<String>("output");
    let export_mime: Option<String> = matches.get_one::<String>("mime-type").cloned();
    let dry_run = matches.get_flag("dry-run");

    // Validate export mime-type for dangerous characters if provided
    if let Some(mime) = &export_mime {
        crate::validate::reject_dangerous_chars(mime, "--mime-type")?;
    }

    // 1. Dry-run: short-circuit before any auth or network I/O, consistent with
    //    how +upload handles --dry-run (auth is attempted optionally, then skipped).
    if dry_run {
        let out_display = output_arg.map(|s| s.as_str()).unwrap_or("<Drive filename>");
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "dry_run": true,
                "fileId": file_id,
                "output": out_display,
                "exportMimeType": export_mime,
            }))
            .unwrap_or_default()
        );
        return Ok(());
    }

    // Resolve methods and scopes from the Discovery Document so that the correct
    // OAuth scopes are used and any custom root_url (proxy, VPC-SC, etc.) is respected.
    let files_res = doc
        .resources
        .get("files")
        .ok_or_else(|| GwsError::Discovery("Resource 'files' not found".to_string()))?;
    let get_method = files_res
        .methods
        .get("get")
        .ok_or_else(|| GwsError::Discovery("Method 'files.get' not found".to_string()))?;
    let export_method = files_res
        .methods
        .get("export")
        .ok_or_else(|| GwsError::Discovery("Method 'files.export' not found".to_string()))?;

    // Union scopes from files.get and files.export to cover both download paths.
    let mut scope_set: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for s in &get_method.scopes {
        scope_set.insert(s.as_str());
    }
    for s in &export_method.scopes {
        scope_set.insert(s.as_str());
    }
    let scopes: Vec<&str> = scope_set.into_iter().collect();

    let token = auth::get_token(&scopes)
        .await
        .map_err(|e| GwsError::Auth(format!("Drive auth failed: {e}")))?;

    // Resolve base URL from the Discovery Document (respects custom root_url / proxy configs).
    let base_url = doc
        .base_url
        .clone()
        .unwrap_or_else(|| format!("{}{}", doc.root_url, doc.service_path));

    let client = crate::client::build_client()?;

    // 2. Fetch file metadata to get name and MIME type
    let encoded_id = crate::validate::encode_path_segment(file_id);
    let metadata_url = format!("{}/files/{}", base_url.trim_end_matches('/'), encoded_id);
    let meta_resp = crate::client::send_with_retry(|| {
        let mut req = client
            .get(&metadata_url)
            .query(&[("fields", "name,mimeType")])
            .bearer_auth(&token);
        if let Some(qp) = crate::auth::get_quota_project() {
            req = req.header("x-goog-user-project", qp);
        }
        req
    })
    .await
    .map_err(|e| GwsError::Other(anyhow::anyhow!("Failed to fetch file metadata: {e}")))?;

    if !meta_resp.status().is_success() {
        let status = meta_resp.status();
        let body = meta_resp.text().await.unwrap_or_default();
        return Err(executor::api_error_from_response(status, &body, &executor::AuthMethod::OAuth));
    }

    let meta: Value = meta_resp
        .json()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Failed to parse file metadata: {e}")))?;

    let drive_name = meta
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("download");
    let mime_type = meta
        .get("mimeType")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let is_google_native = mime_type.starts_with("application/vnd.google-apps.");

    // For native Google Workspace files, --mime-type is required.
    if is_google_native && export_mime.is_none() {
        return Err(GwsError::Validation(format!(
            "The file is a Google Workspace native file ({mime_type}). \
             Provide --mime-type to choose an export format, e.g. \
             --mime-type application/pdf or --mime-type text/csv"
        )));
    }

    // Sanitize drive filename for use as a local path component:
    // - Replace Unix/Windows path separators ('/', '\') to prevent subdirectory traversal.
    // - Replace Windows-reserved characters (':', '*', '?', '"', '<', '>', '|') that
    //   cause file creation failures on Windows.
    // - Strip control characters (Cc category) and dangerous Unicode (Cf/bidi/zero-width)
    //   that could cause terminal injection or misleading output.
    // Note: TOCTOU race conditions on path components are a known limitation;
    // full mitigation via openat(O_NOFOLLOW) is out of scope for this change.
    let safe_name: String = drive_name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
                || c.is_control()
                || crate::validate::is_dangerous_unicode(c)
            {
                '_'
            } else {
                c
            }
        })
        .collect();

    // 3. Resolve and validate output path
    let out_str = output_arg.map(|s| s.as_str()).unwrap_or(&safe_name);
    let out_path = crate::validate::validate_safe_file_path(out_str, "--output")?;

    // Actual MIME type of the file on disk (export format for native files, original otherwise)
    let output_mime = if is_google_native {
        export_mime.as_deref().unwrap_or(mime_type).to_string()
    } else {
        mime_type.to_string()
    };

    // 4. Fetch file content — native Google Workspace files require export;
    //    everything else uses alt=media.
    let resp = if is_google_native {
        // Safety: export_mime is validated as Some above for native files.
        let mime = export_mime.as_deref().unwrap();
        // Build export URL from Discovery Document base URL (respects custom root_url).
        let export_url = format!("{}/files/{}/export", base_url.trim_end_matches('/'), encoded_id);
        let r = crate::client::send_with_retry(|| {
            let mut req = client
                .get(&export_url)
                .query(&[("mimeType", mime)])
                .bearer_auth(&token);
            if let Some(qp) = crate::auth::get_quota_project() {
                req = req.header("x-goog-user-project", qp);
            }
            req
        })
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Drive export request failed: {e}")))?;

        if !r.status().is_success() {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            return Err(executor::api_error_from_response(
                status,
                &body,
                &executor::AuthMethod::OAuth,
            ));
        }
        r
    } else {
        let r = crate::client::send_with_retry(|| {
            let mut req = client
                .get(&metadata_url)
                .query(&[("alt", "media")])
                .bearer_auth(&token);
            if let Some(qp) = crate::auth::get_quota_project() {
                req = req.header("x-goog-user-project", qp);
            }
            req
        })
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Drive download request failed: {e}")))?;

        if !r.status().is_success() {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            return Err(executor::api_error_from_response(
                status,
                &body,
                &executor::AuthMethod::OAuth,
            ));
        }
        r
    };

    // 5. Stream to a temp file first; rename on success to avoid leaving partial files on disk.
    // Include a random suffix to prevent symlink attacks in world-writable directories.
    let tmp_path = out_path.with_file_name(format!(
        ".{}.{:016x}.tmp",
        out_path.file_name().unwrap_or_default().to_string_lossy(),
        rand::random::<u64>()
    ));
    let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
        GwsError::Other(anyhow::anyhow!(
            "Failed to create temp file '{}': {e}",
            tmp_path.display()
        ))
    })?;
    let mut byte_count = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                drop(file);
                let _ = tokio::fs::remove_file(&tmp_path).await;
                return Err(GwsError::Other(anyhow::anyhow!("Download stream error: {e}")));
            }
        };
        byte_count += chunk.len() as u64;
        if let Err(e) = file.write_all(&chunk).await {
            drop(file);
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(GwsError::Other(anyhow::anyhow!(
                "Failed to write to '{}': {e}",
                tmp_path.display()
            )));
        }
    }
    if let Err(e) = file.flush().await {
        drop(file);
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(GwsError::Other(anyhow::anyhow!(
            "Failed to flush '{}': {e}",
            tmp_path.display()
        )));
    }
    drop(file);
    // tokio::fs::rename overwrites the destination atomically on Unix, and on
    // Windows it uses MOVEFILE_REPLACE_EXISTING (supported since Rust 1.26), so
    // a separate remove_file step is unnecessary and would only widen the window
    // where neither the old nor new file exists at the target path.
    if let Err(e) = tokio::fs::rename(&tmp_path, &out_path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(GwsError::Other(anyhow::anyhow!(
            "Failed to finalize download ('{}' -> '{}'): {e}",
            tmp_path.display(),
            out_path.display()
        )));
    }

    // 6. Print result as JSON (consistent with other helper output)
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "savedFile": out_path.display().to_string(),
            "bytes": byte_count,
            "mimeType": output_mime,
        }))
        .unwrap_or_default()
    );

    Ok(())
}

fn determine_filename(file_path: &str, name_arg: Option<&str>) -> Result<String, GwsError> {
    if let Some(n) = name_arg {
        Ok(n.to_string())
    } else {
        Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| GwsError::Validation("Invalid file path".to_string()))
    }
}

fn build_metadata(filename: &str, parent_id: Option<&str>) -> Result<Value, GwsError> {
    let mut metadata = json!({
        "name": filename
    });

    if let Some(parent) = parent_id {
        crate::validate::validate_resource_name(parent)?;
        metadata["parents"] = json!([parent]);
    }

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_filename_explicit() {
        assert_eq!(
            determine_filename("path/to/file.txt", Some("custom.txt")).unwrap(),
            "custom.txt"
        );
    }

    #[test]
    fn test_determine_filename_from_path() {
        assert_eq!(
            determine_filename("path/to/file.txt", None).unwrap(),
            "file.txt"
        );
    }

    #[test]
    fn test_determine_filename_invalid_path() {
        assert!(determine_filename("", None).is_err());
        assert!(determine_filename("/", None).is_err()); // Root has no filename component usually
    }

    #[test]
    fn test_build_metadata_no_parent() {
        let meta = build_metadata("file.txt", None).unwrap();
        assert_eq!(meta["name"], "file.txt");
        assert!(meta.get("parents").is_none());
    }

    #[test]
    fn test_build_metadata_with_parent() {
        let meta = build_metadata("file.txt", Some("folder123")).unwrap();
        assert_eq!(meta["name"], "file.txt");
        assert_eq!(meta["parents"][0], "folder123");
    }

    #[test]
    fn test_build_metadata_rejects_traversal_parent_id() {
        assert!(
            build_metadata("file.txt", Some("../../.ssh/id_rsa")).is_err(),
            "path traversal in --parent must be rejected"
        );
    }

    #[test]
    fn test_build_metadata_rejects_query_injection_parent_id() {
        assert!(
            build_metadata("file.txt", Some("folder?evil=1")).is_err(),
            "'?' in --parent must be rejected"
        );
    }

    #[test]
    fn test_download_command_injected() {
        let helper = DriveHelper;
        let cmd = Command::new("test");
        let doc = crate::discovery::RestDescription::default();
        let cmd = helper.inject_commands(cmd, &doc);
        let names: Vec<_> = cmd.get_subcommands().map(|s| s.get_name()).collect();
        assert!(names.contains(&"+upload"));
        assert!(names.contains(&"+download"));
    }
}
