use crate::{
    services::{ServiceManager, ServiceRecord, now_ms},
    tor::TorManager,
};
use age::secrecy::SecretString;
use age::{Decryptor, Encryptor, Identity};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::{AppHandle, State};
use uuid::Uuid;

const BACKUP_VERSION: u32 = 1;
const MIN_PASSPHRASE_LENGTH: usize = 8;
const MAX_PASSPHRASE_LENGTH: usize = 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    version: u32,
    created_at: u64,
    app_version: String,
    services: Vec<ServiceRecord>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub services: usize,
    pub created_at: u64,
    pub path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSummary {
    pub services: usize,
    pub created_at: u64,
}

#[tauri::command]
pub fn export_backup(
    services: State<'_, Arc<ServiceManager>>,
    path: String,
    passphrase: String,
) -> Result<BackupSummary, String> {
    validate_passphrase(&passphrase)?;
    let path = backup_path(&path);
    let records = services.backup_records();
    let backup = BackupFile {
        version: BACKUP_VERSION,
        created_at: now_ms(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        services: records,
    };
    let ciphertext = encrypt_backup(&backup, &passphrase)?;
    write_private_file(&path, &ciphertext)?;

    Ok(BackupSummary {
        services: backup.services.len(),
        created_at: backup.created_at,
        path: path.display().to_string(),
    })
}

#[tauri::command]
pub async fn import_backup(
    app: AppHandle,
    tor: State<'_, Arc<TorManager>>,
    services: State<'_, Arc<ServiceManager>>,
    path: String,
    passphrase: String,
) -> Result<RestoreSummary, String> {
    validate_passphrase(&passphrase)?;
    let bytes = fs::read(&path)
        .map_err(|error| format!("Could not read the backup file at {path}: {error}"))?;
    let backup = decrypt_backup(&bytes, &passphrase)?;

    if backup.version != BACKUP_VERSION {
        return Err(format!(
            "Unsupported backup version {}. This build supports version {}.",
            backup.version, BACKUP_VERSION
        ));
    }

    let services = services.inner().clone();
    let tor = tor.inner().clone();
    let views = services.replace_all(&app, &tor, backup.services).await?;
    services.sync(&app, &tor).await;

    Ok(RestoreSummary {
        services: views.len(),
        created_at: backup.created_at,
    })
}

fn validate_passphrase(passphrase: &str) -> Result<(), String> {
    if passphrase.chars().count() < MIN_PASSPHRASE_LENGTH {
        return Err(format!(
            "Use a passphrase with at least {MIN_PASSPHRASE_LENGTH} characters."
        ));
    }
    if passphrase.chars().count() > MAX_PASSPHRASE_LENGTH {
        return Err("The backup passphrase is too long.".to_string());
    }
    if passphrase.trim().is_empty() {
        return Err("The backup passphrase cannot be empty.".to_string());
    }
    Ok(())
}

fn backup_path(value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.extension().is_none() {
        path.with_extension("onionhoster")
    } else {
        path
    }
}

fn encrypt_backup(backup: &BackupFile, passphrase: &str) -> Result<Vec<u8>, String> {
    let plaintext = serde_json::to_vec(backup)
        .map_err(|error| format!("Could not serialize the backup: {error}"))?;
    let encryptor = Encryptor::with_user_passphrase(SecretString::new(passphrase.into()));
    let mut ciphertext = Vec::new();
    {
        let mut writer = encryptor
            .wrap_output(&mut ciphertext)
            .map_err(|error| format!("Could not start backup encryption: {error}"))?;
        writer
            .write_all(&plaintext)
            .map_err(|error| format!("Could not encrypt the backup: {error}"))?;
        writer
            .finish()
            .map_err(|error| format!("Could not finish backup encryption: {error}"))?;
    }
    Ok(ciphertext)
}

fn decrypt_backup(bytes: &[u8], passphrase: &str) -> Result<BackupFile, String> {
    let decryptor = Decryptor::new(bytes).map_err(|error| {
        format!("The backup is not a valid encrypted OnionHoster file: {error}")
    })?;
    let identity = age::scrypt::Identity::new(SecretString::new(passphrase.into()));
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn Identity))
        .map_err(|_| "The backup passphrase is incorrect.".to_string())?;
    let mut plaintext = Vec::new();
    reader
        .read_to_end(&mut plaintext)
        .map_err(|error| format!("Could not decrypt the backup: {error}"))?;
    serde_json::from_slice(&plaintext)
        .map_err(|error| format!("The decrypted backup is not valid OnionHoster data: {error}"))
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "The backup path has no parent folder.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Could not create the backup directory at {}: {error}",
            parent.display()
        )
    })?;

    let temporary = parent.join(format!(".onionhoster-{}.tmp", Uuid::new_v4()));
    fs::write(&temporary, bytes).map_err(|error| {
        format!(
            "Could not write the backup to {}: {error}",
            temporary.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            format!("Could not protect the backup file permissions: {error}")
        })?;
    }

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "Could not replace the backup file at {}: {error}",
            path.display()
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{OnionRecord, ServiceSource};

    fn backup() -> BackupFile {
        BackupFile {
            version: BACKUP_VERSION,
            created_at: 42,
            app_version: "0.1.0".to_string(),
            services: vec![ServiceRecord {
                id: "service-1".to_string(),
                name: "Test".to_string(),
                source: ServiceSource::Static {
                    root: "/tmp/site".to_string(),
                    spa_fallback: false,
                    directory_listing: false,
                },
                onion: OnionRecord {
                    virtual_port: 80,
                    service_id: Some(
                        "abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrstuvwxyz234567"
                            .to_string(),
                    ),
                    private_key: Some("ED25519-V3:test".to_string()),
                    pow: true,
                    max_streams: Some(4),
                    private: false,
                    client_auth: Vec::new(),
                },
                autostart: true,
                created_at: 1,
                updated_at: 2,
            }],
        }
    }

    #[test]
    fn encrypted_backup_round_trips() {
        let original = backup();
        let ciphertext =
            encrypt_backup(&original, "correct horse battery staple").expect("encrypt");
        assert!(!ciphertext.windows(5).any(|window| window == b"Test"));
        let decrypted =
            decrypt_backup(&ciphertext, "correct horse battery staple").expect("decrypt");
        assert_eq!(decrypted.version, original.version);
        assert_eq!(decrypted.services.len(), 1);
        assert_eq!(decrypted.services[0].name, "Test");
        assert_eq!(
            decrypted.services[0].onion.private_key.as_deref(),
            Some("ED25519-V3:test")
        );
    }

    #[test]
    fn rejects_the_wrong_passphrase() {
        let ciphertext =
            encrypt_backup(&backup(), "correct horse battery staple").expect("encrypt");
        assert!(decrypt_backup(&ciphertext, "incorrect horse battery staple").is_err());
    }

    #[test]
    fn requires_a_reasonable_passphrase() {
        assert!(validate_passphrase("short").is_err());
        assert!(validate_passphrase("long enough").is_ok());
    }
}
