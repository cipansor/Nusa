//! Direktori/berkas sementara privat untuk artefak build dan query SMT.
//!
//! Nama dibuat unik dengan `RandomState` + waktu + PID, lalu direktori dibuat
//! atomik dengan mode 0700. Jangan pernah memakai kembali direktori yang sudah
//! ada: itu membuka jalan bagi pengguna lokal lain untuk menanam artefak.

use std::fs::{self, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static URUT: AtomicU64 = AtomicU64::new(0);

fn akhiran() -> String {
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u32(std::process::id());
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.write_u64(URUT.fetch_add(1, Ordering::Relaxed));
    format!("{:016x}", h.finish())
}

/// Direktori sementara yang dibuat sendiri dan dihapus saat dijatuhkan.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn baru(awalan: &str) -> io::Result<Self> {
        let dasar = std::env::temp_dir();
        for _ in 0..64 {
            let path = dasar.join(format!("{}-{}", awalan, akhiran()));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(TempDir { path }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "tidak dapat membuat direktori sementara unik",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Menulis berkas baru dengan mode 0600. `create_new` menolak path yang sudah
/// ada, termasuk symlink yang ditanam pengguna lain.
pub fn tulis_privat(path: &Path, isi: &[u8]) -> io::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(isi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn direktori_privat_tidak_dapat_ditulis_pengguna_lain() {
        let d = TempDir::baru("nusa-tmp-uji").unwrap();
        let mode = fs::metadata(d.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode & 0o077, 0, "mode direktori = {:o}", mode);
        let f = d.path().join("data");
        tulis_privat(&f, b"aman").unwrap();
        let mode = fs::metadata(&f).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode & 0o077, 0, "mode berkas = {:o}", mode);
        assert!(tulis_privat(&f, b"timpa").is_err());
    }
}
