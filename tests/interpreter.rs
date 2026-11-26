use nusa::driver::{bangun, cari_cc, OpsiDriver};
use nusa::interp::{jalankan as tafsir_program, Opsi};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn ada_llvm() -> bool {
    let cc = cari_cc();
    Command::new(cc)
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("clang"))
        .unwrap_or(false)
}

fn dir_uji(nama: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "nusa-tafsir-{}-{}-{}",
        std::process::id(),
        nama,
        nanos
    ))
}

fn analisis_ok(sumber: &str) -> nusa::HasilAnalisis {
    nusa::analisis(sumber)
        .unwrap_or_else(|d| panic!("program harus valid:\n{}", d.render(sumber, "uji.ns")))
}

/// Menjalankan lewat interpreter; hasilnya dipetakan seperti CLI.
fn tafsir(sumber: &str, opsi: Opsi) -> (i32, String, String) {
    let a = analisis_ok(sumber);
    let h = tafsir_program(&a.program, opsi);
    match h.kesalahan {
        None => (0, h.keluaran, String::new()),
        Some(k) if k.internal => (
            70,
            h.keluaran,
            format!("nusa: galat internal: {}\n", k.pesan),
        ),
        Some(k) => (134, h.keluaran, format!("nusa: panik: {}\n", k.pesan)),
    }
}

/// Menjalankan lewat compiler LLVM untuk membandingkan perilaku.
fn kompilasi(sumber: &str, nama: &str, opsi: OpsiDriver) -> (i32, String, String) {
    let dir = dir_uji(nama);
    fs::create_dir_all(&dir).unwrap();
    let biner = dir.join("program");
    let h = bangun(sumber, "uji.ns", &biner, &opsi)
        .unwrap_or_else(|d| panic!("gagal membangun:\n{}", d.render(sumber, "uji.ns")));
    let keluaran = Command::new(&biner)
        .output()
        .expect("program gagal dijalankan");
    let _ = fs::remove_dir_all(&h.dir);
    let _ = fs::remove_dir_all(&dir);
    (
        keluaran.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&keluaran.stdout).to_string(),
        String::from_utf8_lossy(&keluaran.stderr).to_string(),
    )
}

#[test]
fn interpreter_fitur_inti() {
    let sumber = r#"
fungsi faktorial(n: bil) -> bil
    syarat n >= 0
    jamin hasil >= 1
{
    jika n <= 1 {
        kembalikan 1
    }
    kembalikan n * faktorial(n - 1)
}

fungsi utama() -> kosong {
    cetakln(faktorial(5))
    var d: daftar<bil> = [1, 2, 3]
    d.tambah(4)
    var e: daftar<bil> = d
    e[0] = 9
    e.tambah(5)
    cetakln(d, " ", e)
    untuk x dalam d {
        jika x == 2 { lanjutkan }
        cetak(x, ";")
    }
    cetakln("")
    var i: bil = 0
    selama i < 3 {
        i += 1
        jika i == 2 { lanjutkan }
        cetak(i)
    }
    cetakln("")
    var s: teks = "Nusa"
    s += "!"
    cetakln(s, " ", panjang(s), " ", s == "Nusa!", " ", "a" < "b")
    cetakln(7 / -3, " ", -7 % 3, " ", 1.5, " ", 1.0)
    cetakln("é😀", " ", panjang("é😀"), " ", 1.0 / 3.0, " ", 1.0e15)
    var n: daftar<daftar<bil>> = [[1], [2]]
    cetakln(n)
}
"#;
    let (kode, out, err) = tafsir(sumber, Opsi::default());
    assert_eq!(kode, 0, "stderr: {}", err);
    assert_eq!(
        out,
        concat!(
            "120\n",
            "[1, 2, 3, 4] [9, 2, 3, 4, 5]\n",
            "1;3;4;",
            "\n13\n",
            "Nusa! 5 benar benar\n",
            "-2 2 1.5 1.0\n",
            "é😀 6 0.333333333333333 1e+15\n",
            "[[1], [2]]\n"
        )
    );
}

#[test]
fn interpreter_memiliki_semantik_nilai_dan_kontrak() {
    let sumber = r#"
fungsi bagi(a: bil, b: bil) -> bil
    syarat b != 0
    jamin hasil * b <= a
{
    kembalikan a / b
}

fungsi utama() -> kosong {
    var a: daftar<bil> = [1]
    var b: daftar<bil> = a
    b.tambah(2)
    cetakln(a, " ", b)
    cetakln(bagi(-7, 3))
}
"#;
    let (kode, out, err) = tafsir(sumber, Opsi::default());
    assert_eq!(kode, 0, "stderr: {}", err);
    assert_eq!(out, "[1] [1, 2]\n-3\n");

    let (kode, _, err) = tafsir(
        "fungsi utama() -> kosong { cetakln(1 / 0) }\n",
        Opsi::default(),
    );
    assert_eq!(kode, 134);
    assert_eq!(err, "nusa: panik: pembagian dengan nol\n");

    let (kode, out, err) = tafsir(
        "fungsi utama() -> kosong { cetakln(\"a\"); tegaskan(2 < 1); cetakln(\"b\") }\n",
        Opsi {
            aman: true,
            kontrak: false,
        },
    );
    assert_eq!(kode, 0, "stderr: {}", err);
    assert_eq!(out, "a\nb\n");
}

#[test]
fn interpreter_panic_sama_dengan_kompiler() {
    if !ada_llvm() {
        eprintln!("dilewati: clang tidak tersedia");
        return;
    }
    let kasus = [
        (
            "div",
            "fungsi utama() -> kosong { cetakln(1); cetakln(1 / 0) }\n",
            OpsiDriver::default(),
        ),
        (
            "batas",
            "fungsi utama() -> kosong { var d: daftar<bil> = [1]\n cetakln(d[3]) }\n",
            OpsiDriver::default(),
        ),
        (
            "luapan",
            "fungsi utama() -> kosong { var x: bil = 9223372036854775807\n cetakln(x + 1) }\n",
            OpsiDriver::default(),
        ),
        (
            "syarat",
            "fungsi f(x: bil) -> bil syarat x > 0 { kembalikan x }\nfungsi utama() -> kosong { cetakln(f(-1)) }\n",
            OpsiDriver::default(),
        ),
        (
            "jamin",
            "fungsi f(x: bil) -> bil jamin hasil > 0 { kembalikan x }\nfungsi utama() -> kosong { cetakln(f(0)) }\n",
            OpsiDriver::default(),
        ),
        (
            "tegaskan",
            "fungsi utama() -> kosong { cetakln(\"a\"); tegaskan(2 < 1) }\n",
            OpsiDriver::default(),
        ),
        (
            "mutlak",
            "fungsi utama() -> kosong { var x: bil = -9223372036854775807 - 1\n cetakln(mutlak(x)) }\n",
            OpsiDriver::default(),
        ),
    ];
    for (nama, sumber, opsi) in kasus {
        let (kc, oc, ec) = kompilasi(sumber, nama, opsi.clone());
        let (ki, oi, ei) = tafsir(
            sumber,
            Opsi {
                aman: opsi.aman,
                kontrak: opsi.kontrak,
            },
        );
        assert_eq!(kc, ki, "kode keluar berbeda pada {}", nama);
        assert_eq!(oc, oi, "stdout berbeda pada {}", nama);
        assert_eq!(ec, ei, "stderr berbeda pada {}", nama);
    }

    // `--lepas` mematikan cek luapan di kedua mode.
    let sumber = "fungsi utama() -> kosong { var x: bil = 9223372036854775807\n cetakln(x + 1) }\n";
    let opsi_lepas = OpsiDriver {
        aman: false,
        kontrak: false,
        ..OpsiDriver::default()
    };
    let (kc, oc, _) = kompilasi(sumber, "lepas", opsi_lepas);
    assert_eq!(kc, 0);
    let opsi = Opsi {
        aman: false,
        kontrak: false,
    };
    let (ki, oi, _) = tafsir(sumber, opsi);
    assert_eq!(ki, 0);
    assert_eq!(oc, oi);
}

#[test]
fn interpreter_dan_kompiler_menghasilkan_keluaran_sama_untuk_semua_contoh() {
    if !ada_llvm() {
        eprintln!("dilewati: clang tidak tersedia");
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("contoh");
    let mut berkas = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "ns"))
        .collect::<Vec<_>>();
    berkas.sort();
    assert!(!berkas.is_empty());
    for p in berkas {
        let sumber = fs::read_to_string(&p).unwrap();
        let nama = p.file_stem().unwrap().to_string_lossy().to_string();
        let (kc, oc, ec) = kompilasi(&sumber, &nama, OpsiDriver::default());
        let (ki, oi, ei) = tafsir(&sumber, Opsi::default());
        assert_eq!(kc, ki, "kode keluar berbeda pada {}", p.display());
        assert_eq!(oc, oi, "stdout berbeda pada {}", p.display());
        assert_eq!(ec, ei, "stderr berbeda pada {}", p.display());
    }
}

#[test]
fn cli_menyediakan_mode_kompilasi_dan_interpretasi() {
    let dir = dir_uji("cli");
    fs::create_dir_all(&dir).unwrap();
    let berkas = dir.join("halo.ns");
    fs::write(
        &berkas,
        "fungsi utama() -> kosong { cetakln(\"Halo, Nusa!\") }\n",
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_nusa");

    for perintah in [
        vec!["interpretasi", berkas.to_str().unwrap()],
        vec!["tafsir", berkas.to_str().unwrap()],
        vec!["jalankan", berkas.to_str().unwrap(), "--tafsir"],
    ] {
        let o = Command::new(bin).args(&perintah).output().unwrap();
        assert_eq!(
            o.status.code(),
            Some(0),
            "perintah {:?}: {}",
            perintah,
            String::from_utf8_lossy(&o.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&o.stdout), "Halo, Nusa!\n");
    }

    if ada_llvm() {
        let o = Command::new(bin)
            .arg("jalankan")
            .arg(&berkas)
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(0));
        assert_eq!(String::from_utf8_lossy(&o.stdout), "Halo, Nusa!\n");
    }
    let _ = fs::remove_dir_all(&dir);
}
