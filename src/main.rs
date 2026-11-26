use nusa::diag::Diag;
use nusa::driver::{self, OpsiDriver};
use std::path::{Path, PathBuf};
use std::process::exit;

const BANTUAN: &str = r#"Nusa (.ns) — bahasa Indonesia, aman memori, compiler LLVM + interpreter

PEMAKAIAN:
  nusa <perintah> <berkas.ns> [opsi]

PERINTAH:
  bangun      kompilasi .ns menjadi biner
  jalankan    kompilasi lalu jalankan
  interpretasi jalankan langsung tanpa LLVM (interpreter/tree-walking)
  periksa     periksa sintaks, tipe, dan keamanan tanpa membangun
  ir          cetak LLVM IR hasil kompilasi
  verifikasi  buktikan kontrak `syarat`/`jamin` dengan SMT (z3)
  bantuan     tampilkan pesan ini

OPSI:
  -o, --keluaran <jalan>   lokasi biner keluaran
  --lepas                  optimisasi penuh, tanpa cek kontrak/luapan
  --tanpa-kontrak          matikan pemeriksaan kontrak saat berjalan
  --tanpa-aman             matikan pemeriksaan luapan aritmetika
  --tafsir                 pada `jalankan`: pakai interpreter, bukan compiler
  -v, --verbose            tampilkan perintah tautan

OPSI VERIFIKASI:
  --tanpa-z3               hanya tulis berkas SMT-LIB
  --keluaran-dir <dir>     direktori berkas bukti (default: bukti)

CONTOH:
  nusa jalankan contoh/halo.ns
  nusa interpretasi contoh/halo.ns
  nusa jalankan contoh/halo.ns --tafsir
  nusa bangun contoh/faktorial.ns -o faktorial
  nusa verifikasi contoh/kontrak.ns
"#;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print!("{}", BANTUAN);
        exit(0);
    }
    match args[0].as_str() {
        "bantuan" | "bantu" | "help" | "-h" | "--help" => {
            print!("{}", BANTUAN);
        }
        "versi" | "version" | "-V" | "--version" => {
            println!("nusa {}", env!("CARGO_PKG_VERSION"));
        }
        "bangun" | "build" => perintah_bangun(&args[1..]),
        "jalankan" | "run" => perintah_jalankan(&args[1..]),
        "interpretasi" | "tafsir" | "interpreter" | "interp" => perintah_interpretasi(&args[1..]),
        "periksa" | "check" => perintah_periksa(&args[1..]),
        "ir" => perintah_ir(&args[1..]),
        "verifikasi" | "verify" | "bukti" => nusa::verify::perintah(&args[1..]),
        lain => {
            eprintln!("nusa: perintah `{}` tidak dikenal", lain);
            eprintln!("jalankan `nusa bantuan`");
            exit(2);
        }
    }
}

struct Argumen {
    berkas: PathBuf,
    keluaran: Option<PathBuf>,
    opsi: OpsiDriver,
    tafsir: bool,
}

fn urai(args: &[String], perlu_berkas: bool) -> Argumen {
    let mut berkas = None;
    let mut keluaran = None;
    let mut opsi = OpsiDriver::default();
    let mut tafsir = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "-o" | "--keluaran" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("nusa: opsi `{}` membutuhkan jalan", a);
                    exit(2);
                }
                keluaran = Some(PathBuf::from(&args[i]));
            }
            "--lepas" | "--release" => {
                opsi.aman = false;
                opsi.kontrak = false;
                opsi.optimasi = "3".into();
            }
            "--tanpa-kontrak" => opsi.kontrak = false,
            "--tanpa-aman" => opsi.aman = false,
            "--tafsir" | "--interpretasi" => tafsir = true,
            "-v" | "--verbose" => opsi.verbose = true,
            lain if lain.starts_with('-') => {
                eprintln!("nusa: opsi `{}` tidak dikenal", lain);
                exit(2);
            }
            lain => {
                if berkas.is_some() {
                    eprintln!("nusa: hanya satu berkas sumber yang diterima");
                    exit(2);
                }
                berkas = Some(PathBuf::from(lain));
            }
        }
        i += 1;
    }
    let berkas = match berkas {
        Some(b) => b,
        None if perlu_berkas => {
            eprintln!("nusa: berkas .ns belum diberikan");
            eprintln!("jalankan `nusa bantuan`");
            exit(2);
        }
        None => PathBuf::new(),
    };
    Argumen {
        berkas,
        keluaran,
        opsi,
        tafsir,
    }
}

fn laporkan(d: Diag, berkas: &Path) -> ! {
    let sumber = std::fs::read_to_string(berkas).unwrap_or_default();
    eprint!("{}", d.render(&sumber, &berkas.display().to_string()));
    exit(1)
}

fn perintah_bangun(args: &[String]) {
    let a = urai(args, true);
    match driver::bangun_ke_berkas(&a.berkas, a.keluaran.as_deref(), &a.opsi) {
        Ok(h) => {
            let _ = std::fs::remove_dir_all(&h.dir);
            println!("{}", h.biner.display());
        }
        Err(d) => laporkan(d, &a.berkas),
    }
}

fn perintah_jalankan(args: &[String]) {
    let a = urai(args, true);
    if a.tafsir {
        perintah_interpretasi(args);
        return;
    }
    match driver::jalankan(&a.berkas, &a.opsi) {
        Ok(kode) => exit(kode),
        Err(d) => laporkan(d, &a.berkas),
    }
}

fn perintah_interpretasi(args: &[String]) {
    let a = urai(args, true);
    let sumber = std::fs::read_to_string(&a.berkas).unwrap_or_else(|e| {
        eprintln!("nusa: tidak dapat membaca `{}`: {}", a.berkas.display(), e);
        exit(1);
    });
    let anal = match nusa::analisis(&sumber) {
        Ok(a) => a,
        Err(d) => laporkan(d, &a.berkas),
    };
    let hasil = nusa::interp::jalankan(
        &anal.program,
        nusa::interp::Opsi {
            aman: a.opsi.aman,
            kontrak: a.opsi.kontrak,
        },
    );
    use std::io::Write;
    print!("{}", hasil.keluaran);
    let _ = std::io::stdout().flush();
    if let Some(k) = hasil.kesalahan {
        if k.internal {
            eprintln!("nusa: galat internal: {}", k.pesan);
            exit(70);
        }
        eprintln!("nusa: panik: {}", k.pesan);
        exit(134);
    }
}

fn perintah_periksa(args: &[String]) {
    let a = urai(args, true);
    match driver::periksa(&a.berkas) {
        Ok(pesan) => println!("{}", pesan),
        Err(d) => laporkan(d, &a.berkas),
    }
}

fn perintah_ir(args: &[String]) {
    let a = urai(args, true);
    let sumber = std::fs::read_to_string(&a.berkas).unwrap_or_else(|e| {
        eprintln!("nusa: tidak dapat membaca `{}`: {}", a.berkas.display(), e);
        exit(1);
    });
    match nusa::kompilasi_ir(
        &sumber,
        nusa::codegen::Opsi {
            aman: a.opsi.aman,
            kontrak: a.opsi.kontrak,
        },
    ) {
        Ok((k, _)) => {
            if let Some(p) = &a.keluaran {
                if let Err(e) = std::fs::write(p, &k.ir) {
                    eprintln!("nusa: tidak dapat menulis `{}`: {}", p.display(), e);
                    exit(1);
                }
            } else {
                print!("{}", k.ir);
            }
        }
        Err(d) => laporkan(d, &a.berkas),
    }
}
