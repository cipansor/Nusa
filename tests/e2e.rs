use nusa::driver::{bangun, cari_cc, OpsiDriver};
use std::fs;
use std::path::PathBuf;
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
        "nusa-uji-{}-{}-{}",
        std::process::id(),
        nama,
        nanos
    ))
}

fn jalankan(sumber: &str, nama: &str, opsi: OpsiDriver) -> (i32, String, String) {
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
fn halo_dunia() {
    if !ada_llvm() {
        eprintln!("dilewati: clang tidak tersedia");
        return;
    }
    let (kode, out, _) = jalankan(
        "fungsi utama() -> kosong {\n cetakln(\"Halo, Nusa!\")\n}\n",
        "halo",
        OpsiDriver::default(),
    );
    assert_eq!(kode, 0);
    assert_eq!(out, "Halo, Nusa!\n");
}

#[test]
fn fitur_inti_bahasa() {
    if !ada_llvm() {
        eprintln!("dilewati: clang tidak tersedia");
        return;
    }
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
    cetakln(d)
    untuk x dalam d {
        cetak(x, " ")
    }
    cetakln("")
    var s: teks = "Nusa"
    s += "!"
    cetakln(s + " kuat")
    var i: bil = 0
    selama i < 4 {
        i += 1
        jika i == 2 { lanjutkan }
        cetak(i, " ")
    }
    cetakln("")
    cetakln(-7 / 3)
    cetakln(-7 % 3)
    cetakln(1.5 * 2)
}
"#;
    let (kode, out, err) = jalankan(sumber, "fitur", OpsiDriver::default());
    assert_eq!(kode, 0, "stderr: {}", err);
    assert_eq!(
        out,
        "120\n[1, 2, 3, 4]\n1 2 3 4 \nNusa! kuat\n1 3 4 \n-3\n2\n3.0\n"
    );
}

#[test]
fn semantik_nilai_dan_pengembalian() {
    if !ada_llvm() {
        eprintln!("dilewati: clang tidak tersedia");
        return;
    }
    let sumber = r#"
fungsi ubah(d: daftar<bil>) -> kosong {
    d.tambah(100)
    cetakln(d)
}

fungsi buat(n: bil) -> daftar<bil> {
    var d: daftar<bil> = []
    untuk i dalam 0..n {
        d.tambah(i * i)
    }
    kembalikan d
}

fungsi utama() -> kosong {
    var a: daftar<bil> = [1, 2]
    var b: daftar<bil> = a
    b.tambah(3)
    cetakln(a)
    cetakln(b)
    ubah(a)
    cetakln(a)
    cetakln(buat(4))
}
"#;
    let (kode, out, err) = jalankan(sumber, "nilai", OpsiDriver::default());
    assert_eq!(kode, 0, "stderr: {}", err);
    assert_eq!(
        out,
        "[1, 2]\n[1, 2, 3]\n[1, 2, 100]\n[1, 2]\n[0, 1, 4, 9]\n"
    );
}

#[test]
fn batas_daftar_diperiksa() {
    if !ada_llvm() {
        eprintln!("dilewati: clang tidak tersedia");
        return;
    }
    let (kode, _, err) = jalankan(
        "fungsi utama() -> kosong {\n var d: daftar<bil> = [1]\n cetakln(d[3])\n}\n",
        "batas",
        OpsiDriver::default(),
    );
    assert_eq!(kode, 134);
    assert!(err.contains("indeks di luar batas"), "stderr: {}", err);
}

#[test]
fn luapan_diperiksa_dan_bisa_dimatikan() {
    if !ada_llvm() {
        eprintln!("dilewati: clang tidak tersedia");
        return;
    }
    let sumber =
        "fungsi utama() -> kosong {\n var x: bil = 9223372036854775807\n x += 1\n cetakln(x)\n}\n";
    let (kode, _, err) = jalankan(sumber, "luapan", OpsiDriver::default());
    assert_eq!(kode, 134);
    assert!(err.contains("luapan"), "stderr: {}", err);

    let lepas = OpsiDriver {
        aman: false,
        kontrak: false,
        optimasi: "3".into(),
        verbose: false,
    };
    let (kode2, out2, _) = jalankan(sumber, "luapan2", lepas);
    assert_eq!(kode2, 0);
    assert_eq!(out2, "-9223372036854775808\n");
}

#[test]
fn kontrak_diperiksa_saat_berjalan() {
    if !ada_llvm() {
        eprintln!("dilewati: clang tidak tersedia");
        return;
    }
    let sumber = r#"
fungsi positif(x: bil) -> bil
    jamin hasil > 0
{
    kembalikan x
}
fungsi utama() -> kosong {
    cetakln(positif(0))
}
"#;
    let (kode, _, err) = jalankan(sumber, "kontrak", OpsiDriver::default());
    assert_eq!(kode, 134);
    assert!(err.contains("jamin"), "stderr: {}", err);
    let tanpa = OpsiDriver {
        aman: true,
        kontrak: false,
        optimasi: "2".into(),
        verbose: false,
    };
    let (kode2, out2, _) = jalankan(sumber, "kontrak2", tanpa);
    assert_eq!(kode2, 0);
    assert_eq!(out2, "0\n");
}

#[test]
fn aliasing_elemen_dan_unicode() {
    if !ada_llvm() {
        eprintln!("dilewati: clang tidak tersedia");
        return;
    }
    let sumber = r#"
fungsi utama() -> kosong {
    var π: bil = 3
    var d: daftar<teks> = ["a", "b", "c"]
    d[0] = d[1]
    cetakln(d)
    d.tambah(d[2])
    cetakln(d)
    var s: teks = "é😀"
    s += s
    cetakln(π, " ", s, " ", panjang(s))
    var n: daftar<daftar<bil>> = [[1], [2]]
    n.tambah(n[0])
    n[0] = n[1]
    cetakln(n)
}
"#;
    let (kode, out, err) = jalankan(sumber, "alias", OpsiDriver::default());
    assert_eq!(kode, 0, "stderr: {}", err);
    assert_eq!(
        out,
        "[b, b, c]\n[b, b, c, c]\n3 é😀é😀 12\n[[2], [2], [1]]\n"
    );
}

#[test]
fn verifikasi_kontrak_dengan_z3() {
    if !ada_llvm() {
        eprintln!("dilewati: clang tidak tersedia");
        return;
    }
    if Command::new("z3").arg("--version").output().is_err() {
        eprintln!("dilewati: z3 tidak tersedia");
        return;
    }
    let sumber = r#"
fungsi tambah_loop(n: bil) -> bil
    syarat n >= 0
    jamin hasil >= 0
{
    var t: bil = 0
    var i: bil = 0
    selama i < n invarian t >= 0 dan i >= 0 dan i <= n {
        i += 1
        t += 1
    }
    kembalikan t
}
fungsi keliru(a: bil) -> bil
    jamin hasil > 0
{
    kembalikan a
}
fungsi rusak(n: bil) -> bil
    syarat n >= 0
    jamin hasil >= 0
{
    var t: bil = 1
    var i: bil = 0
    selama i < n invarian t == i + 1 {
        t += 2
        i += 1
    }
    kembalikan t
}
fungsi lewat_break(n: bil) -> bil
    syarat n >= 0
    jamin hasil == n
{
    var i: bil = 0
    selama i < n invarian i >= 0 dan i <= n {
        jika i == 1 { hentikan }
        i += 1
    }
    kembalikan i
}
fungsi utama() -> kosong { cetakln(0) }
"#;
    let a = nusa::analisis(sumber).expect("program harus valid");
    let dir = dir_uji("bukti");
    let laporan = nusa::verify::verifikasi_program(&a.program, &a.info, &dir, true);
    let cari = |n: &str| laporan.fungsi.iter().find(|f| f.nama == n).unwrap();
    assert_eq!(cari("tambah_loop").status, nusa::verify::Status::Terbukti);
    assert_eq!(cari("keliru").status, nusa::verify::Status::Gagal);
    assert_eq!(cari("rusak").status, nusa::verify::Status::Gagal);
    assert_eq!(cari("lewat_break").status, nusa::verify::Status::Gagal);
    let _ = fs::remove_dir_all(&dir);
}
