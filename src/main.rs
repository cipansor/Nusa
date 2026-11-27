//! Nusa: kompiler & penafsir bahasa Indonesia yang aman memori, aman tipe,
//! cepat, kecil, dan dapat diverifikasi formal.

mod ast;
mod diag;
mod interp;
mod lex;
mod llvm;
mod parse;
mod sema;
mod smt;
mod tmp;

use std::io::{IsTerminal, Write};
use std::process::{Command, Stdio};
use std::time::Instant;

const RT_C: &str = include_str!("../runtime/nusa_rt.c");

struct Opsi {
    berkas: String,
    keluaran: Option<String>,
    bebas_jaga: bool,
    simpan_ir: bool,
    opt: u8,
    boleh_takaman: bool,
}

fn main() {
    let utas = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(utama)
        .expect("gagal membuat utas utama");
    std::process::exit(utas.join().unwrap_or(2));
}

fn utama() -> i32 {
    let arg: Vec<String> = std::env::args().skip(1).collect();
    if arg.is_empty() || arg[0] == "-h" || arg[0] == "--bantuan" || arg[0] == "bantuan" {
        bantuan();
        return if arg.is_empty() { 2 } else { 0 };
    }
    let perintah = if matches!(
        arg[0].as_str(),
        "bangun" | "periksa" | "ir" | "jalan" | "jalankan" | "bukti" | "--smt"
    ) {
        arg[0].clone()
    } else {
        "bangun".to_string()
    };
    let sisa: Vec<String> = if perintah == arg[0] {
        arg[1..].to_vec()
    } else {
        arg.clone()
    };
    let Some(berkas) = sisa
        .iter()
        .find(|a| !a.starts_with('-') && a.ends_with(".ns"))
        .cloned()
        .or_else(|| sisa.first().cloned())
    else {
        eprintln!("nusa: sebutkan berkas .ns\n");
        bantuan();
        return 2;
    };
    let mut opsi = Opsi {
        berkas,
        keluaran: None,
        bebas_jaga: false,
        simpan_ir: false,
        opt: 2,
        boleh_takaman: false,
    };
    let mut i = 0;
    while i < sisa.len() {
        match sisa[i].as_str() {
            "-o" | "--keluaran" => {
                i += 1;
                if i < sisa.len() {
                    opsi.keluaran = Some(sisa[i].clone());
                }
            }
            "--bebas-jaga" => opsi.bebas_jaga = true,
            "--boleh-takaman" => opsi.boleh_takaman = true,
            "--simpan-ir" => opsi.simpan_ir = true,
            "--opt" => {
                i += 1;
                opsi.opt = sisa.get(i).and_then(|s| s.parse().ok()).unwrap_or(2);
            }
            _ => {}
        }
        i += 1;
    }
    match perintah.as_str() {
        "periksa" => periksa_saja(&opsi),
        "ir" => cetak_ir(&opsi),
        "bukti" | "--smt" => buktikan(&opsi),
        "jalan" => tafsirkan(&opsi),
        "jalankan" => jalankan(&opsi),
        _ => bangun(&opsi),
    }
}

fn bantuan() {
    println!(
        "Nusa 0.1 — kompiler & penafsir bahasa Indonesia, backend LLVM

PEMAKAIAN:
    nusa <perintah> berkas.ns [opsi]

DUA CARA EKSEKUSI:
    jalan      jalankan langsung sebagai PENAFSIR (tanpa biner)
    bangun     KOMPILASI ke biner native lewat LLVM (bawaan)
    jalankan   kompilasi lalu jalankan binernya

PERINTAH LAIN:
    periksa    periksa tipe, kepemilikan, dan peminjaman (cepat)
    ir         tampilkan LLVM IR ke stdout
    bukti      buktikan kontrak perlu/jamin/invarian/tegas dengan Z3
    --smt      nama lain untuk `bukti` (SMT/Z3)

OPSI:
    -o <jalur>   nama berkas keluaran (mode kompilasi)
    --opt N      tingkat optimasi LLVM 0..3 (bawaan 2)
    --simpan-ir  simpan berkas .ll di samping biner
    --bebas-jaga buang penjaga runtime saat kompilasi, hanya bila semua tugas terbukti
    --boleh-takaman izinkan blok `takaman` (mematikan penjaga runtime di blok itu)

Contoh:
    nusa jalan  contoh/pencarian.ns
    nusa bangun contoh/pencarian.ns -o cari && ./cari"
    );
}

struct Dimuat {
    prog: ast::Prog,
    sema: sema::Sema,
}

fn cetak_diag(d: &[diag::Diagnostik], sumber: &str) {
    let berwarna = std::io::stderr().is_terminal();
    for x in d {
        x.cetak(sumber, berwarna);
    }
}

fn muat(o: &Opsi) -> Result<Dimuat, i32> {
    let berkas = &o.berkas;
    let mut prog = ast::Prog::default();
    let mut dikunjungi = std::collections::HashSet::new();
    let mut id_berikut = 1;
    muat_berkas(berkas, &mut prog, &mut dikunjungi, &mut id_berikut)?;
    let sumber = std::fs::read_to_string(berkas).unwrap_or_default();
    let s = sema::periksa(&prog, berkas, o.boleh_takaman);
    if diag::ada_galat(&s.diag) {
        cetak_diag(&s.diag, &sumber);
        return Err(1);
    }
    if !s.diag.is_empty() {
        cetak_diag(&s.diag, &sumber);
    }
    Ok(Dimuat { prog, sema: s })
}

fn muat_berkas(
    berkas: &str,
    prog: &mut ast::Prog,
    dikunjungi: &mut std::collections::HashSet<std::path::PathBuf>,
    id_berikut: &mut u32,
) -> Result<(), i32> {
    let kunci = std::fs::canonicalize(berkas).unwrap_or_else(|_| std::path::PathBuf::from(berkas));
    if !dikunjungi.insert(kunci) {
        return Ok(());
    }
    let sumber = match std::fs::read_to_string(berkas) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("nusa: tidak dapat membaca `{}`: {}", berkas, e);
            return Err(2);
        }
    };
    let h = lex::lentik(&sumber, berkas);
    let mut d = h.diag;
    let u = parse::urai_mulai(&h.token, berkas, *id_berikut);
    *id_berikut = u.id_berikut;
    d.extend(u.diag);
    if diag::ada_galat(&d) {
        cetak_diag(&d, &sumber);
        return Err(1);
    }
    let induk = std::path::Path::new(berkas)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    for nama in &u.prog.impor {
        let mentah = std::path::Path::new(nama);
        let jalur = if mentah.is_absolute() {
            mentah.to_path_buf()
        } else {
            induk.join(mentah)
        };
        let jalur = if jalur.extension().is_some() {
            jalur
        } else {
            jalur.with_extension("ns")
        };
        if !jalur.is_file() {
            let sp = ast::Span::default();
            let d = diag::Diagnostik::galat(
                berkas,
                sp,
                format!(
                    "modul `{}` tidak ditemukan (dicari di `{}`)",
                    nama,
                    jalur.display()
                ),
                "E0116",
            );
            cetak_diag(&[d], &sumber);
            return Err(1);
        }
        muat_berkas(&jalur.to_string_lossy(), prog, dikunjungi, id_berikut)?;
    }
    prog.fungsi.extend(u.prog.fungsi);
    prog.struk.extend(u.prog.struk);
    prog.enumerasi.extend(u.prog.enumerasi);
    prog.alias.extend(u.prog.alias);
    Ok(())
}

fn periksa_saja(o: &Opsi) -> i32 {
    match muat(o) {
        Ok(d) => {
            println!(
                "nusa: `{}` lolos pemeriksaan tipe, kepemilikan, dan peminjaman",
                o.berkas
            );
            println!(
                "  {} fungsi, {} struk, {} enumerasi",
                d.prog.fungsi.len(),
                d.prog.struk.len(),
                d.prog.enumerasi.len()
            );
            0
        }
        Err(k) => k,
    }
}

fn cetak_ir(o: &Opsi) -> i32 {
    match muat(o) {
        Ok(d) => {
            let cg = llvm::Codegen::baru(
                &d.prog,
                &d.sema,
                llvm::Opsi {
                    jaga: true,
                    opt: "2",
                },
            );
            print!("{}", cg.hasil());
            0
        }
        Err(k) => k,
    }
}

fn buktikan(o: &Opsi) -> i32 {
    let d = match muat(o) {
        Ok(d) => d,
        Err(k) => return k,
    };
    let lap = smt::periksa(&d.prog, &d.sema, &o.berkas);
    lap.cetak();
    if lap.gagal > 0 || lap.tidak_diketahui > 0 || lap.dilewati > 0 {
        1
    } else {
        0
    }
}

fn jalankan(o: &Opsi) -> i32 {
    let tmp = match tmp::TempDir::baru("nusa-jalan") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("nusa: gagal membuat direktori sementara: {}", e);
            return 2;
        }
    };
    let keluar = tmp.path().join("program");
    let o2 = Opsi {
        berkas: o.berkas.clone(),
        keluaran: Some(keluar.to_string_lossy().to_string()),
        bebas_jaga: o.bebas_jaga,
        simpan_ir: false,
        opt: o.opt,
        boleh_takaman: o.boleh_takaman,
    };
    let kode = bangun_diam(&o2);
    if kode != 0 {
        return kode;
    }
    let status = Command::new(&keluar).status();
    match status {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("nusa: gagal menjalankan biner: {}", e);
            2
        }
    }
}

fn jalur_alat(kandidat: &[&str]) -> Option<String> {
    for k in kandidat {
        if Command::new(k)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(k.to_string());
        }
    }
    None
}

/// Runtime selalu dirakit di direktori build privat. Cache bersama di
/// `/tmp` tidak dipakai: objek dan penanda di sana dapat diganti pengguna
/// lokal lain sebelum ditautkan ke biner.
fn objek_runtime(dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let c = dir.join("nusa_rt.c");
    let obj = dir.join("nusa_rt.o");
    tmp::tulis_privat(&c, RT_C.as_bytes()).map_err(|e| format!("tulis runtime: {}", e))?;
    let cc = jalur_alat(&["cc", "gcc", "clang"]).ok_or("tidak ada `cc`/`gcc` untuk runtime")?;
    let hasil = Command::new(cc)
        .args(["-O2", "-c"])
        .arg(&c)
        .arg("-o")
        .arg(&obj)
        .status()
        .map_err(|e| format!("jalankan cc: {}", e))?;
    if !hasil.success() {
        return Err("gagal merakit runtime C".into());
    }
    Ok(obj)
}

fn tafsirkan(o: &Opsi) -> i32 {
    let d = match muat(o) {
        Ok(d) => d,
        Err(k) => return k,
    };
    let _ = writeln!(
        std::io::stderr(),
        "nusa: menafsirkan `{}` (tanpa biner)",
        o.berkas
    );
    let _ = std::io::stdout().flush();
    interp::jalan(&d.prog, &d.sema, true)
}

fn bangun_diam(o: &Opsi) -> i32 {
    let t0 = Instant::now();
    let d = match muat(o) {
        Ok(d) => d,
        Err(k) => return k,
    };
    if o.bebas_jaga {
        let lap = smt::periksa(&d.prog, &d.sema, &o.berkas);
        lap.cetak();
        if lap.gagal > 0 || lap.tidak_diketahui > 0 || lap.dilewati > 0 {
            eprintln!(
                "nusa: `--bebas-jaga` ditolak: masih ada tugas yang belum terbukti atau fungsi yang dilewati"
            );
            return 1;
        }
    }
    let cg = llvm::Codegen::baru(
        &d.prog,
        &d.sema,
        llvm::Opsi {
            jaga: !o.bebas_jaga,
            opt: "2",
        },
    );
    let ir = cg.hasil();
    let keluar = o
        .keluaran
        .clone()
        .unwrap_or_else(|| o.berkas.trim_end_matches(".ns").to_string());
    let tmp = match tmp::TempDir::baru("nusa-bangun") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("nusa: gagal membuat direktori build sementara: {}", e);
            return 2;
        }
    };
    let ir_path = tmp.path().join("program.ll");
    if tmp::tulis_privat(&ir_path, ir.as_bytes()).is_err() {
        eprintln!("nusa: gagal menulis IR sementara");
        return 2;
    }
    if o.simpan_ir {
        let _ = std::fs::write(format!("{}.ll", keluar), &ir);
    }
    let alat = match jalur_alat(&["llc-19", "llc-18", "llc", "clang-19", "clang"]) {
        Some(a) => a,
        None => {
            eprintln!("nusa: butuh `llc` atau `clang` di PATH");
            return 2;
        }
    };
    let obj = tmp.path().join("program.o");
    let mut cmd = Command::new(&alat);
    if alat.starts_with("llc") {
        cmd.args([
            "-filetype=obj",
            "-relocation-model=pic",
            &format!("-O{}", o.opt),
        ])
        .arg(&ir_path)
        .arg("-o")
        .arg(&obj);
    } else {
        cmd.args([&format!("-O{}", o.opt), "-c"])
            .arg(&ir_path)
            .arg("-o")
            .arg(&obj);
    }
    let status = cmd.status();
    match status {
        Ok(s) if s.success() => {}
        _ => {
            eprintln!("nusa: perakitan LLVM gagal");
            return 2;
        }
    }
    let rt = match objek_runtime(tmp.path()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("nusa: {}", e);
            return 2;
        }
    };
    let cc = jalur_alat(&["cc", "gcc", "clang"]).unwrap_or_else(|| "cc".into());
    let status = Command::new(cc)
        .arg(&obj)
        .arg(&rt)
        .arg("-o")
        .arg(&keluar)
        .status();
    match status {
        Ok(s) if s.success() => {}
        _ => {
            eprintln!("nusa: penautan gagal");
            return 2;
        }
    }
    let ukuran = std::fs::metadata(&keluar).map(|m| m.len()).unwrap_or(0);
    let ms = t0.elapsed().as_millis();
    let _ = std::io::stdout().flush();
    println!(
        "nusa: `{}` -> `{}` ({} fungsi, {:.1} KB, {} ms{})",
        o.berkas,
        keluar,
        d.prog.fungsi.len(),
        ukuran as f64 / 1024.0,
        ms,
        if o.bebas_jaga { ", tanpa jaga" } else { "" }
    );
    0
}

fn bangun(o: &Opsi) -> i32 {
    bangun_diam(o)
}
