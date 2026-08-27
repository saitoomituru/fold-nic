// SPDX-License-Identifier: AGPL-3.0-or-later

//! CIDv1／SHA2-256でbyte列を同定し、専用root内へatomic publishするlocal CAS。

#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use cid::Cid;
use multihash_codetable::{Code, MultihashDigest};

const RAW_CODEC: u64 = 0x55;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StoreErrorCode {
    UnsafeRoot,
    ObjectTooLarge,
    ObjectNotFound,
    Io,
    CorruptObject,
}

impl StoreErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsafeRoot => "CAS_UNSAFE_ROOT",
            Self::ObjectTooLarge => "CAS_OBJECT_TOO_LARGE",
            Self::ObjectNotFound => "CAS_OBJECT_NOT_FOUND",
            Self::Io => "CAS_IO_ERROR",
            Self::CorruptObject => "CAS_CORRUPT_OBJECT",
        }
    }
}

#[derive(Debug)]
pub struct StoreError {
    pub code: StoreErrorCode,
    pub detail: String,
    source: Option<std::io::Error>,
}

impl StoreError {
    fn plain(code: StoreErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
            source: None,
        }
    }

    fn io(detail: impl Into<String>, source: std::io::Error) -> Self {
        Self {
            code: StoreErrorCode::Io,
            detail: detail.into(),
            source: Some(source),
        }
    }
}

impl Display for StoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_ref().map(|error| error as &dyn Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PutReceipt {
    pub cid: Cid,
    pub size: u64,
    pub reused_existing: bool,
}

#[derive(Debug, Clone)]
pub struct LocalCas {
    root: PathBuf,
    objects_root: PathBuf,
    max_object_bytes: u64,
}

impl LocalCas {
    /// 専用CAS rootを作成し、canonical pathへ固定する。
    ///
    /// # Errors
    ///
    /// rootがsymlink、directory以外、作成不能の場合に返す。
    pub fn open(root: impl AsRef<Path>, max_object_bytes: u64) -> Result<Self, StoreError> {
        let requested_root = root.as_ref();
        if max_object_bytes == 0 {
            return Err(StoreError::plain(
                StoreErrorCode::ObjectTooLarge,
                "max_object_bytesは1以上である必要があります",
            ));
        }
        if let Ok(metadata) = fs::symlink_metadata(requested_root) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(StoreError::plain(
                    StoreErrorCode::UnsafeRoot,
                    "CAS rootはsymlinkではないdirectoryである必要があります",
                ));
            }
        } else {
            create_private_directory(requested_root)?;
        }

        let root = requested_root
            .canonicalize()
            .map_err(|error| StoreError::io("CAS rootをcanonicalizeできません", error))?;
        let objects_root = root.join("objects");
        create_private_directory(&objects_root)?;
        reject_symlink(&objects_root)?;

        Ok(Self {
            root,
            objects_root,
            max_object_bytes,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub const fn max_object_bytes(&self) -> u64 {
        self.max_object_bytes
    }

    /// byte列をCIDで同定し、既存objectを上書きせずatomic publishする。
    ///
    /// # Errors
    ///
    /// size上限超過、filesystem error、既存object破損の場合に返す。
    pub fn put(&self, bytes: &[u8]) -> Result<PutReceipt, StoreError> {
        let size = u64::try_from(bytes.len()).map_err(|_| {
            StoreError::plain(
                StoreErrorCode::ObjectTooLarge,
                "object sizeをu64で表現できません",
            )
        })?;
        if size > self.max_object_bytes {
            return Err(StoreError::plain(
                StoreErrorCode::ObjectTooLarge,
                format!(
                    "object size {size}は上限{}を超えています",
                    self.max_object_bytes
                ),
            ));
        }

        let cid = cid_for(bytes);
        let path = self.object_path(&cid);
        if path.exists() {
            self.verify_path(&path, &cid)?;
            return Ok(PutReceipt {
                cid,
                size,
                reused_existing: true,
            });
        }

        let parent = path.parent().ok_or_else(|| {
            StoreError::plain(StoreErrorCode::UnsafeRoot, "object parentを解決できません")
        })?;
        create_private_directory(parent)?;
        reject_symlink(parent)?;

        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            cid,
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = open_private_new(&temporary)?;
        file.write_all(bytes)
            .map_err(|error| StoreError::io("temporary objectを書き込めません", error))?;
        file.sync_data()
            .map_err(|error| StoreError::io("temporary objectをsyncできません", error))?;
        drop(file);

        match fs::hard_link(&temporary, &path) {
            Ok(()) => {
                remove_temporary(&temporary)?;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                remove_temporary(&temporary)?;
                self.verify_path(&path, &cid)?;
                return Ok(PutReceipt {
                    cid,
                    size,
                    reused_existing: true,
                });
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                return Err(StoreError::io("objectをatomic publishできません", error));
            }
        }

        Ok(PutReceipt {
            cid,
            size,
            reused_existing: false,
        })
    }

    /// CIDに対応するbyte列を読み、返す前にdigestを再検証する。
    ///
    /// # Errors
    ///
    /// object不在、size上限超過、symlink、digest不一致、I/O errorの場合に返す。
    pub fn get(&self, cid: &Cid) -> Result<Vec<u8>, StoreError> {
        let path = self.object_path(cid);
        reject_symlink(&path)?;
        let mut file = File::open(&path).map_err(|error| {
            if error.kind() == ErrorKind::NotFound {
                StoreError::plain(StoreErrorCode::ObjectNotFound, "CAS objectがありません")
            } else {
                StoreError::io("CAS objectを開けません", error)
            }
        })?;
        let metadata = file
            .metadata()
            .map_err(|error| StoreError::io("CAS object metadataを読めません", error))?;
        if !metadata.is_file() || metadata.len() > self.max_object_bytes {
            return Err(StoreError::plain(
                StoreErrorCode::CorruptObject,
                "CAS objectが通常fileではないかsize上限を超えています",
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
        file.read_to_end(&mut bytes)
            .map_err(|error| StoreError::io("CAS objectを読めません", error))?;
        if cid_for(&bytes) != *cid {
            return Err(StoreError::plain(
                StoreErrorCode::CorruptObject,
                "CAS objectのCID再計算が一致しません",
            ));
        }
        Ok(bytes)
    }

    #[must_use]
    pub fn object_path(&self, cid: &Cid) -> PathBuf {
        let encoded = cid.to_string();
        let shard = &encoded[..encoded.len().min(2)];
        self.objects_root.join(shard).join(encoded)
    }

    fn verify_path(&self, path: &Path, cid: &Cid) -> Result<(), StoreError> {
        reject_symlink(path)?;
        self.get(cid).map(|_| ())
    }
}

#[must_use]
pub fn cid_for(bytes: &[u8]) -> Cid {
    Cid::new_v1(RAW_CODEC, Code::Sha2_256.digest(bytes))
}

fn reject_symlink(path: &Path) -> Result<(), StoreError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            StoreError::plain(StoreErrorCode::ObjectNotFound, "CAS objectがありません")
        } else {
            StoreError::io("path metadataを読めません", error)
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(StoreError::plain(
            StoreErrorCode::UnsafeRoot,
            "symlinkはCAS pathとして利用できません",
        ));
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> Result<(), StoreError> {
    if path.exists() {
        return Ok(());
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder
        .create(path)
        .map_err(|error| StoreError::io("private directoryを作成できません", error))
}

fn open_private_new(path: &Path) -> Result<File, StoreError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options
        .open(path)
        .map_err(|error| StoreError::io("temporary objectを作成できません", error))
}

fn remove_temporary(path: &Path) -> Result<(), StoreError> {
    fs::remove_file(path).map_err(|error| StoreError::io("temporary objectを削除できません", error))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "fold-store-test-{label}-{}-{nonce}",
                std::process::id()
            ));
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn put_getで同じbytesを返す() {
        let root = TestRoot::new("roundtrip");
        let cas = LocalCas::open(&root.0, 1024).expect("open CAS");
        let receipt = cas.put(b"fold-nic-stage0").expect("put object");
        assert!(!receipt.reused_existing);
        assert_eq!(
            cas.get(&receipt.cid).expect("get object"),
            b"fold-nic-stage0"
        );
    }

    #[test]
    fn 同じobjectをdeduplicateする() {
        let root = TestRoot::new("dedupe");
        let cas = LocalCas::open(&root.0, 1024).expect("open CAS");
        let first = cas.put(b"same").expect("first put");
        let second = cas.put(b"same").expect("second put");
        assert_eq!(first.cid, second.cid);
        assert!(second.reused_existing);
    }

    #[test]
    fn size上限を超えたobjectを拒否する() {
        let root = TestRoot::new("limit");
        let cas = LocalCas::open(&root.0, 3).expect("open CAS");
        let error = cas.put(b"four").expect_err("must reject oversized object");
        assert_eq!(error.code, StoreErrorCode::ObjectTooLarge);
    }

    #[test]
    fn 既存object改変を検出する() {
        let root = TestRoot::new("corrupt");
        let cas = LocalCas::open(&root.0, 1024).expect("open CAS");
        let receipt = cas.put(b"original").expect("put object");
        fs::write(cas.object_path(&receipt.cid), b"modified").expect("modify fixture");
        let error = cas.get(&receipt.cid).expect_err("must detect corruption");
        assert_eq!(error.code, StoreErrorCode::CorruptObject);
    }

    #[test]
    fn 不在objectをstable_codeで返す() {
        let root = TestRoot::new("not-found");
        let cas = LocalCas::open(&root.0, 1024).expect("open CAS");
        let missing = cid_for(b"missing");
        let error = cas.get(&missing).expect_err("missing object");
        assert_eq!(error.code, StoreErrorCode::ObjectNotFound);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_rootを拒否する() {
        use std::os::unix::fs::symlink;

        let target = TestRoot::new("symlink-target");
        fs::create_dir_all(&target.0).expect("create target");
        let link = TestRoot::new("symlink-link");
        symlink(&target.0, &link.0).expect("create symlink");
        let error = LocalCas::open(&link.0, 1024).expect_err("must reject symlink root");
        assert_eq!(error.code, StoreErrorCode::UnsafeRoot);
    }
}
