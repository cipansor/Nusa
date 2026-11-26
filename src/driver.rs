//! Driver kompilasi: menghasilkan IR, memanggil LLVM (clang), lalu menautkan.

use crate::analisis;
use crate::codegen;
use crate::diag::{Diag, Hasil};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct OpsiDriver {
    pub aman: bool,
    pub kontrak: bool,
    pub optimasi: String,
    pub verbose: bool,
}

impl Default for OpsiDriver {
    fn default() -> Self {
        OpsiDriver {
            aman: true,
            kontrak: true,
            optimasi: "2".into(),
            verbose: false,
        }
    }
}

pub fn cari_cc() -> String {
    if let Ok(cc) = std::env::var("NUSA_CC") {
        if !cc.is_empty() {
            return cc;
        }
    }
    for calon in ["clang-19", "clang", "cc"] {
        if Command::new(calon)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return calon.to_string();
        }
    }
    "cc".into()
}

fn direktori_sementara() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("nusa-{}-{}", std::process::id(), nanos))
}

pub struct HasilBangun {
    pub biner: PathBuf,
    pub dir: PathBuf,
    pub ir: String,
}

/// Mengompilasi sumber Nusa menjadi biner melalui LLVM.
pub fn bangun(
    sumber: &str,
    nama_berkas: &str,
    keluaran: &Path,
    opsi: &OpsiDriver,
) -> Hasil<HasilBangun> {
    let (keluaran_ir, _) = crate::kompilasi_ir(
        sumber,
        codegen::Opsi {
            aman: opsi.aman,
            kontrak: opsi.kontrak,
        },
    )?;
    let dir = direktori_sementara();
    fs::create_dir_all(&dir).map_err(|e| {
        Diag::baru(
            "NUSA-D001",
            format!("tidak dapat membuat direktori sementara: {}", e),
        )
    })?;
    let ir_path = dir.join("program.ll");
    let rt_path = dir.join("nusa_rt.c");
    fs::write(&ir_path, &keluaran_ir.ir)
        .map_err(|e| Diag::baru("NUSA-D002", format!("tidak dapat menulis IR: {}", e)))?;
    fs::write(&rt_path, codegen::RUNTIME_C)
        .map_err(|e| Diag::baru("NUSA-D003", format!("tidak dapat menulis runtime: {}", e)))?;

    let cc = cari_cc();
    let mut cmd = Command::new(&cc);
    cmd.arg(&ir_path)
        .arg(&rt_path)
        .arg("-o")
        .arg(keluaran)
        .arg(format!("-O{}", opsi.optimasi))
        .arg("-Wno-override-module")
        .arg("-fno-unwind-tables")
        .arg("-fno-asynchronous-unwind-tables")
        .arg("-s");
    if opsi.verbose {
        eprintln!(
            "nusa: menjalankan {} {:?}",
            cc,
            cmd.get_args().collect::<Vec<_>>()
        );
    }
    let keluaran_proses = cmd.output().map_err(|e| {
        Diag::baru(
            "NUSA-D004",
            format!("tidak dapat menjalankan `{}`: {}", cc, e),
        )
        .catat("atur variabel NUSA_CC bila clang berada di lokasi lain")
    })?;
    if !keluaran_proses.status.success() {
        let _ = fs::remove_dir_all(&dir);
        let stderr = String::from_utf8_lossy(&keluaran_proses.stderr);
        return Err(Diag::baru(
            "NUSA-D005",
            format!("LLVM/clang gagal menautkan program `{}`", nama_berkas),
        )
        .catat(
            stderr
                .trim()
                .lines()
                .take(12)
                .collect::<Vec<_>>()
                .join(" | "),
        ));
    }

    Ok(HasilBangun {
        biner: keluaran.to_path_buf(),
        dir,
        ir: keluaran_ir.ir,
    })
}

pub fn bangun_ke_berkas(
    berkas: &Path,
    keluaran: Option<&Path>,
    opsi: &OpsiDriver,
) -> Hasil<HasilBangun> {
    let sumber = fs::read_to_string(berkas).map_err(|e| {
        Diag::baru(
            "NUSA-D006",
            format!("tidak dapat membaca `{}`: {}", berkas.display(), e),
        )
    })?;
    let keluar = keluaran
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| berkas.with_extension(""));
    bangun(&sumber, &berkas.display().to_string(), &keluar, opsi)
}

pub fn jalankan(berkas: &Path, opsi: &OpsiDriver) -> Hasil<i32> {
    let h = bangun_ke_berkas(berkas, None, opsi)?;
    let status = Command::new(&h.biner).status().map_err(|e| {
        Diag::baru(
            "NUSA-D007",
            format!("tidak dapat menjalankan `{}`: {}", h.biner.display(), e),
        )
    });
    let _ = fs::remove_dir_all(&h.dir);
    let _ = fs::remove_file(&h.biner);
    let status = status?;
    Ok(status.code().unwrap_or(134))
}

pub fn periksa(berkas: &Path) -> Hasil<String> {
    let sumber = fs::read_to_string(berkas).map_err(|e| {
        Diag::baru(
            "NUSA-D006",
            format!("tidak dapat membaca `{}`: {}", berkas.display(), e),
        )
    })?;
    let a = analisis(&sumber)?;
    let jumlah_kontrak: usize = a
        .program
        .fungsi
        .iter()
        .map(|f| f.syarat.len() + f.jamin.len())
        .sum();
    Ok(format!(
        "ok: {} fungsi, {} kontrak, tipe aman",
        a.program.fungsi.len(),
        jumlah_kontrak
    ))
}
