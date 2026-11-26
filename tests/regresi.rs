use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn nusa() -> &'static str {
    env!("CARGO_BIN_EXE_nusa")
}

fn dir(nama: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nusa-regresi-{}-{}", std::process::id(), nama));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn tulis(d: &Path, nama: &str, isi: &str) -> PathBuf {
    let p = d.join(nama);
    fs::write(&p, isi).unwrap();
    p
}

fn jalankan(args: &[&str]) -> Output {
    Command::new(nusa()).args(args).output().unwrap()
}

fn ada_llc() -> bool {
    ["llc-19", "llc-18", "llc", "clang-19", "clang"]
        .iter()
        .any(|t| {
            Command::new(t)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
}

fn clang() -> Option<&'static str> {
    ["clang-19", "clang"].into_iter().find(|t| {
        Command::new(t)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

fn jalankan_asan(nama: &str, isi: &str) -> Option<String> {
    let clang = clang()?;
    let d = dir(nama);
    let f = tulis(&d, "program.ns", isi);
    let ir = jalankan(&["ir", f.to_str().unwrap()]);
    assert!(ir.status.success(), "{}", teks_galat(&ir));
    let ll = d.join("program.ll");
    fs::write(&ll, &ir.stdout).unwrap();
    let biner = d.join("program-asan");
    let rt = Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/nusa_rt.c");
    let c = Command::new(clang)
        .args(["-fsanitize=address", "-g", "-o"])
        .arg(&biner)
        .arg(&ll)
        .arg(&rt)
        .output()
        .unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    let run = Command::new(&biner)
        .env("ASAN_OPTIONS", "detect_leaks=1")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "biner ASAN gagal: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let galat = String::from_utf8_lossy(&run.stderr);
    assert!(!galat.contains("LeakSanitizer"), "{galat}");
    assert!(!galat.contains("AddressSanitizer"), "{galat}");
    Some(teks_keluar(&run))
}

fn teks_keluar(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn teks_galat(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn periksa(nama: &str, isi: &str, kode: &str) {
    let d = dir(nama);
    let f = tulis(&d, "program.ns", isi);
    let o = jalankan(&["periksa", f.to_str().unwrap()]);
    assert!(!o.status.success(), "{nama}: seharusnya ditolak");
    assert!(
        teks_galat(&o).contains(kode),
        "{nama}: kode {kode} tidak muncul:\n{}",
        teks_galat(&o)
    );
}

/// Jalankan penafsir; bila LLVM tersedia, bandingkan dengan biner native.
fn sama_di_dua_mesin(nama: &str, isi: &str) -> String {
    let d = dir(nama);
    let f = tulis(&d, "program.ns", isi);
    let i = jalankan(&["jalan", f.to_str().unwrap()]);
    assert!(
        i.status.success(),
        "{nama}: penafsir gagal:\n{}",
        teks_galat(&i)
    );
    let keluaran = teks_keluar(&i);
    if ada_llc() {
        let biner = d.join("program");
        let b = jalankan(&["bangun", f.to_str().unwrap(), "-o", biner.to_str().unwrap()]);
        assert!(
            b.status.success(),
            "{nama}: bangun gagal:\n{}",
            teks_galat(&b)
        );
        let run = Command::new(&biner).output().unwrap();
        assert!(
            run.status.success(),
            "{nama}: biner gagal:\n{}",
            teks_galat(&run)
        );
        assert_eq!(keluaran, teks_keluar(&run), "{nama}: penafsir != biner");
    }
    keluaran
}

#[test]
fn min_i64_dapat_ditulis_dan_dibaca() {
    let k = sama_di_dua_mesin(
        "min-i64",
        r#"
fungsi utama() -> bil {
    ub x: bil = -9223372036854775808;
    cetak("x=", x)
    kembali 0
}
"#,
    );
    assert!(k.contains("x=-9223372036854775808"), "{k}");
}

#[test]
fn min_i64_positif_tetap_ditolak() {
    periksa(
        "min-positif",
        r#"
fungsi utama() -> bil {
    cetak(9223372036854775808)
    kembali 0
}
"#,
        "E0004",
    );
}

#[test]
fn overflow_min_tetap_ditangkap_sebagai_kesalahan_runtime() {
    let d = dir("min-overflow");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi utama() -> bil {
    ub x: bil = -9223372036854775808;
    kembali -x
}
"#,
    );
    let i = jalankan(&["jalan", f.to_str().unwrap()]);
    assert_eq!(i.status.code(), Some(1));
    assert!(teks_galat(&i).contains("luber bilangan"));
}

#[test]
fn min_dibagi_atau_disisa_minus_satu_ditangkap() {
    for (nama, op) in [("bagi", "/"), ("sisa", "%")] {
        let d = dir(nama);
        let f = tulis(
            &d,
            "program.ns",
            &format!(
                "fungsi utama() -> bil {{\n    tetap x: bil = -9223372036854775808;\n    cetak(x {op} -1)\n    kembali 0\n}}\n"
            ),
        );
        let i = jalankan(&["jalan", f.to_str().unwrap()]);
        assert_eq!(
            i.status.code(),
            Some(1),
            "{nama}: penafsir: {}",
            teks_galat(&i)
        );
        assert!(
            teks_galat(&i).contains("luber bilangan"),
            "{nama}: {}",
            teks_galat(&i)
        );
        if ada_llc() {
            let b = d.join("program");
            assert!(
                jalankan(&["bangun", f.to_str().unwrap(), "-o", b.to_str().unwrap()])
                    .status
                    .success()
            );
            let r = Command::new(&b).output().unwrap();
            assert_eq!(r.status.code(), Some(1), "{nama}: biner");
            assert!(
                teks_galat(&r).contains("luber bilangan"),
                "{nama}: {}",
                teks_galat(&r)
            );
        }
    }
}

#[test]
fn konversi_pecah_ke_bulat_dan_sebaliknya() {
    let k = sama_di_dua_mesin(
        "konversi-pecah",
        r#"
fungsi utama() -> bil {
    cetak("a=", 3.99 sebagai bulat, " b=", -3.99 sbg bulat, " c=", 7 sebagai pecahan / 2.0)
    kembali 0
}
"#,
    );
    assert_eq!(k, "a=3 b=-3 c=3.5\n");
}

#[test]
fn konversi_pecah_di_luar_rentang_ditangkap_di_kedua_mesin() {
    let d = dir("pecah-luar");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi utama() -> bil {
    cetak(1.0e30 sebagai bil)
    kembali 0
}
"#,
    );
    let i = jalankan(&["jalan", f.to_str().unwrap()]);
    assert_eq!(i.status.code(), Some(1));
    assert!(teks_galat(&i).contains("luber bilangan"));
    if ada_llc() {
        let b = d.join("program");
        assert!(
            jalankan(&["bangun", f.to_str().unwrap(), "-o", b.to_str().unwrap()])
                .status
                .success()
        );
        let r = Command::new(&b).output().unwrap();
        assert_eq!(r.status.code(), Some(1));
        assert!(teks_galat(&r).contains("luber bilangan"));
    }
}

#[test]
fn geser_di_luar_lebar_ditolak() {
    let d = dir("geser");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi utama() -> bil {
    ub x: bil = 1;
    cetak(x << 64)
    kembali 0
}
"#,
    );
    let i = jalankan(&["jalan", f.to_str().unwrap()]);
    assert_eq!(i.status.code(), Some(1));
    assert!(teks_galat(&i).contains("geser tidak sah"));
}

#[test]
fn takaman_mematikan_penjaga_dan_wajib_opt_in() {
    let src = r#"
fungsi utama() -> bil {
    ub x: bil = 9223372036854775807;
    takaman { x = x + 1 }
    cetak("x=", x)
    kembali 0
}
"#;
    periksa("takaman-tanpa-izin", src, "E0385");
    let d = dir("takaman");
    let f = tulis(&d, "program.ns", src);
    let i = jalankan(&["jalan", f.to_str().unwrap(), "--boleh-takaman"]);
    assert!(i.status.success(), "{}", teks_galat(&i));
    assert_eq!(teks_keluar(&i), "x=-9223372036854775808\n");
    if ada_llc() {
        let b = d.join("program");
        let c = jalankan(&[
            "bangun",
            f.to_str().unwrap(),
            "--boleh-takaman",
            "-o",
            b.to_str().unwrap(),
        ]);
        assert!(c.status.success(), "{}", teks_galat(&c));
        let r = Command::new(&b).output().unwrap();
        assert!(r.status.success());
        assert_eq!(teks_keluar(&r), "x=-9223372036854775808\n");
    }
}

#[test]
fn cocok_harus_lengkap_dan_varian_harus_ada() {
    periksa(
        "cocok-lengkap",
        r#"
pilihan A { X, Y }
fungsi f(a: A) -> bil {
    cocok a { kasus X { kembali 1 } }
}
fungsi utama() -> bil { kembali 0 }
"#,
        "E0382",
    );
    periksa(
        "cocok-varian",
        r#"
pilihan A { X, Y }
fungsi f(a: A) -> bil {
    cocok a { kasus X { kembali 1 } kasus Z { kembali 2 } }
}
fungsi utama() -> bil { kembali 0 }
"#,
        "E0380",
    );
}

#[test]
fn metode_pada_nilai_tetap_ditolak() {
    periksa(
        "statik",
        r#"
fungsi utama() -> bil {
    tetap d: Daftar<bil> = daftar();
    d.tambah(1)
    kembali d.panjang()
}
"#,
        "E0383",
    );
}

#[test]
fn kedalaman_ekspresi_dibatasi_tanpa_crash() {
    let dalam = format!("{}benar", "!".repeat(20000));
    periksa(
        "kedalaman",
        &format!("fungsi utama() -> bil {{ cetak({dalam}) kembali 0 }}\n"),
        "E0115",
    );
}

#[test]
fn rekursi_penafsir_dibatasi_tanpa_stack_overflow() {
    let d = dir("rekursi");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi turun(n: bil) -> bil {
    jika n == 0 { kembali 0 }
    kembali 1 + turun(n - 1)
}
fungsi utama() -> bil { kembali turun(5000) }
"#,
    );
    let i = jalankan(&["jalan", f.to_str().unwrap()]);
    assert_eq!(i.status.code(), Some(1));
    assert!(teks_galat(&i).contains("rekursi terlalu dalam"));
}

#[test]
fn impor_modul_dan_duplikasi_deklarasi() {
    let d = dir("impor");
    tulis(
        &d,
        "ilmu.ns",
        "fungsi tambah_dua(x: bil) -> bil { kembali x + 2 }\n",
    );
    let utama = tulis(
        &d,
        "utama.ns",
        r#"
impor "ilmu"
fungsi utama() -> bil {
    cetak("hasil=", tambah_dua(5))
    kembali 0
}
"#,
    );
    let i = jalankan(&["jalan", utama.to_str().unwrap()]);
    assert!(i.status.success(), "{}", teks_galat(&i));
    assert_eq!(teks_keluar(&i), "hasil=7\n");
    if ada_llc() {
        let b = d.join("utama");
        let c = jalankan(&["bangun", utama.to_str().unwrap(), "-o", b.to_str().unwrap()]);
        assert!(c.status.success(), "{}", teks_galat(&c));
        let r = Command::new(&b).output().unwrap();
        assert!(r.status.success(), "{}", teks_galat(&r));
        assert_eq!(teks_keluar(&r), "hasil=7\n");
    }

    let d2 = dir("impor-duplikat");
    tulis(&d2, "a.ns", "fungsi sama() -> bil { kembali 1 }\n");
    tulis(&d2, "b.ns", "fungsi sama() -> bil { kembali 2 }\n");
    let m = tulis(
        &d2,
        "utama.ns",
        "impor \"a\"\nimpor \"b\"\nfungsi utama() -> bil { kembali sama() }\n",
    );
    let o = jalankan(&["periksa", m.to_str().unwrap()]);
    assert!(!o.status.success());
    assert!(teks_galat(&o).contains("E0208"), "{}", teks_galat(&o));
}

#[test]
fn pindah_medan_non_salin_dilacak() {
    periksa(
        "pindah-medan-terlarang",
        r#"
struk S { a: Daftar<bil>, b: bil }
fungsi pakai(d: Daftar<bil>) -> bil { kembali d.panjang() }
fungsi utama() -> bil {
    ub s: S = S { a: daftar(), b: 2 };
    cetak(pakai(s.a))
    cetak(s.a.panjang())
    kembali 0
}
"#,
        "E0320",
    );
    let k = sama_di_dua_mesin(
        "pindah-medan-sah",
        r#"
struk S { a: Daftar<bil>, b: bil }
fungsi pakai(d: Daftar<bil>) -> bil { kembali d.panjang() }
fungsi utama() -> bil {
    ub s: S = S { a: daftar(), b: 42 };
    cetak("a=", pakai(s.a), " b=", s.b)
    kembali 0
}
"#,
    );
    assert_eq!(k, "a=0 b=42\n");
}

#[test]
fn pinjaman_medan_tak_berubah_dan_pindah_medan_bekerja() {
    let k = sama_di_dua_mesin(
        "medan",
        r#"
struk S { a: Daftar<bil>, b: bil }
fungsi isi(d: &Daftar<bil>) -> bil { kembali d.panjang() }
fungsi utama() -> bil {
    ub d: Daftar<bil> = daftar();
    d.tambah(1); d.tambah(2);
    ub s: S = S { a: d, b: 3 };
    cetak("panjang=", isi(&s.a), " b=", s.b)
    kembali 0
}
"#,
    );
    assert_eq!(k, "panjang=2 b=3\n");
}

#[test]
fn contoh_fitur_lengkap_sama_di_dua_mesin() {
    let src = r#"
bentuk Titik { x: bulat, y: bulat }
pilihan Arah { Utara, Selatan, Timur, Barat }
jenis Sisi = cacah32;

fungsi geser(t: Titik, dx: bulat, dy: bulat) -> Titik {
    kembali Titik { x: t.x + dx, y: t.y + dy };
}

fungsi nama_arah(a: Arah) -> teks {
    cocok a {
        kasus Utara { kembali "utara"; }
        kasus Selatan { kembali "selatan"; }
        kasus Timur { kembali "timur"; }
        kasus Barat { kembali "barat"; }
    }
}

fungsi utama() {
    tp asal: Titik = Titik { x: 1, y: 2 };
    tp tujuan: Titik = geser(asal, 10, 20);
    cetak("tujuan:", tujuan.x, tujuan.y);
    ub n: Sisi = 7;
    n += 1;
    cetak("sisi:", n);
    cetak("arah:", nama_arah(Arah.Timur));
}
"#;
    let k = sama_di_dua_mesin("fitur", src);
    assert_eq!(k, "tujuan:1122\nsisi:8\narah:timur\n");
}

#[test]
fn tulis_ke_elemen_larik_tetap_tidak_hilang() {
    let k = sama_di_dua_mesin(
        "larik-tulis",
        r#"
fungsi utama() -> bil {
    ub a: [bil; 3] = [1, 2, 3];
    a[1] = 9;
    cetak(a[0], ",", a[1], ",", a[2]);
    ub b: [pecah; 2] = [1.5, 2.5];
    b[1] = 4.5;
    cetak(b[0], ",", b[1]);
    kembali 0
}
"#,
    );
    assert_eq!(k, "1,9,3\n1.5,4.5\n");
}

#[test]
fn lingkup_blok_dan_variabel_iterasi_tidak_bocor() {
    let k = sama_di_dua_mesin(
        "lingkup",
        r#"
fungsi utama() -> bil {
    ub i: bil = 7;
    {
        ub i: bil = 10;
        cetak("dalam=", i);
    }
    cetak("luar=", i);
    untuk i dalam 0..2 { cetak("iterasi=", i); }
    cetak("setelah=", i);
    kembali 0
}
"#,
    );
    assert_eq!(k, "dalam=10\nluar=7\niterasi=0\niterasi=1\nsetelah=7\n");
}

#[test]
fn iterasi_daftar_memimjam_wadah_selama_badan() {
    periksa(
        "iterasi-ubah-metode",
        r#"
fungsi utama() -> bil {
    ub d: Daftar<bil> = daftar();
    d.tambah(1); d.tambah(2);
    untuk x dalam d { d.tambah(x) }
    kembali d.panjang()
}
"#,
        "E0336",
    );
    periksa(
        "iterasi-ubah-pinjam",
        r#"
fungsi isi(d: &ubah Daftar<bil>, x: bil) { tambah(d, x) }
fungsi utama() -> bil {
    ub d: Daftar<bil> = daftar();
    d.tambah(1); d.tambah(2);
    untuk x dalam d { isi(&ubah d, x) }
    kembali d.panjang()
}
"#,
        "E0333",
    );
}

#[test]
fn impor_modul_dengan_posisi_sama_tidak_tumpang_tindih() {
    let d = dir("impor-id");
    tulis(
        &d,
        "a.ns",
        r#"
fungsi jum_a() -> bil {
    ub s: bil = 0;
    untuk x dalam [1, 2, 3] { s = s + x }
    kembali s
}
"#,
    );
    tulis(
        &d,
        "b.ns",
        r#"
fungsi jum_b() -> pecah {
    ub s: pecah = 0.0;
    untuk y dalam [1.5, 2.5, 3.5] { s = s + y }
    kembali s
}
"#,
    );
    let f = tulis(
        &d,
        "utama.ns",
        r#"
impor "a"
impor "b"
fungsi utama() -> bil {
    cetak("a=", jum_a(), " b=", jum_b());
    kembali 0
}
"#,
    );
    let i = jalankan(&["jalan", f.to_str().unwrap()]);
    assert!(i.status.success(), "{}", teks_galat(&i));
    assert_eq!(teks_keluar(&i), "a=6 b=7.5\n");
    if ada_llc() {
        let biner = d.join("utama");
        let b = jalankan(&["bangun", f.to_str().unwrap(), "-o", biner.to_str().unwrap()]);
        assert!(b.status.success(), "{}", teks_galat(&b));
        let run = Command::new(&biner).output().unwrap();
        assert!(run.status.success(), "{}", teks_galat(&run));
        assert_eq!(teks_keluar(&run), "a=6 b=7.5\n");
    }
}

#[test]
fn bebas_jaga_menolak_fungsi_yang_dilewati() {
    let d = dir("bebas-jaga-lewat");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi pakai(d: Daftar<bil>) -> bil { kembali d.panjang() }
fungsi utama() -> bil {
    ub d: Daftar<bil> = daftar();
    d.tambah(1);
    kembali pakai(d)
}
"#,
    );
    let biner = d.join("program");
    let o = jalankan(&[
        "bangun",
        f.to_str().unwrap(),
        "-o",
        biner.to_str().unwrap(),
        "--bebas-jaga",
    ]);
    assert!(!o.status.success());
    assert!(
        teks_galat(&o).contains("fungsi yang dilewati"),
        "{}",
        teks_galat(&o)
    );
}

fn ada_z3() -> bool {
    Command::new("z3").arg("--version").output().is_ok()
}

#[test]
fn keluar_loop_tidak_vakum_di_verifikator() {
    if !ada_z3() {
        return;
    }
    let d = dir("loop-vakum");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi utama() -> bil {
    untuk i dalam 0..3 { cetak(i) }
    tegas salah
    kembali 0
}
"#,
    );
    let o = jalankan(&["bukti", f.to_str().unwrap()]);
    assert!(!o.status.success());
    let gabung = format!("{}{}", teks_keluar(&o), teks_galat(&o));
    assert!(gabung.contains("`tegas` harus selalu benar"), "{gabung}");
}

#[test]
fn daftar_dibebaskan_pada_kembali_dan_lanjut() {
    let Some(keluaran) = jalankan_asan(
        "bebas-daftar",
        r#"
fungsi pakai(d: Daftar<bil>) -> bil { kembali d.panjang() }
fungsi utama() -> bil {
    ub i: bil = 0;
    selama i < 3 {
        ub d: Daftar<bil> = daftar();
        d.tambah(i);
        cetak(pakai(d));
        i = i + 1;
        lanjut;
    }
    ub e: Daftar<bil> = daftar();
    e.tambah(5);
    cetak(pakai(e));
    kembali 0
}
"#,
    ) else {
        return;
    };
    assert_eq!(keluaran, "1\n1\n1\n1\n");
}

#[test]
fn panggilan_di_kepala_loop_memindahkan_argumen_daftar() {
    let program = r#"
fungsi ukuran(d: Daftar<bil>) -> [bil; 1] {
    kembali [d.panjang()]
}
fungsi utama() -> bil {
    ub d: Daftar<bil> = daftar();
    d.tambah(7);
    untuk x dalam ukuran(d) { cetak(x); }
    kembali 0
}
"#;
    let k = sama_di_dua_mesin("kepala-loop", program);
    assert_eq!(k, "1\n");
    if let Some(k) = jalankan_asan("kepala-loop-asan", program) {
        assert_eq!(k, "1\n");
    }
}

#[test]
fn larik_tersimpul_berisi_daftar_ditolak() {
    periksa(
        "larik-tersimpul-daftar",
        r#"
fungsi utama() -> bil {
    ub d: Daftar<bil> = daftar();
    d.tambah(1);
    ub a = [d];
    ub e = a[0];
    kembali e.panjang()
}
"#,
        "E0203",
    );
}

#[test]
fn indeks_panggilan_pada_penugasan_larik_tetap() {
    let k = sama_di_dua_mesin(
        "indeks-panggilan-tulis",
        r#"
fungsi pilih() -> bil { kembali 1 }
fungsi utama() -> bil {
    ub a: [bil; 2] = [1, 2];
    a[pilih()] = 9;
    cetak(a[0], ",", a[1]);
    kembali 0
}
"#,
    );
    assert_eq!(k, "1,9\n");
}

#[test]
fn indeks_yang_memindahkan_tempat_tujuan_ditolak() {
    periksa(
        "indeks-memindahkan-tujuan",
        r#"
fungsi indeks(d: Daftar<bil>) -> bil { kembali 0 }
fungsi utama() -> bil {
    ub a: Daftar<bil> = daftar();
    a.tambah(1);
    a[indeks(a)] = 9;
    kembali a[0]
}
"#,
        "E0320",
    );
}

#[test]
fn indeks_daftar_boleh_memindahkan_wadah_lain() {
    let program = r#"
fungsi indeks(d: Daftar<bil>) -> bil { kembali d.panjang() }
fungsi utama() -> bil {
    ub a: Daftar<bil> = daftar();
    a.tambah(1); a.tambah(2);
    ub d: Daftar<bil> = daftar();
    d.tambah(3);
    a[indeks(d)] = d.panjang();
    cetak(a[1]);
    kembali 0
}
"#;
    let k = sama_di_dua_mesin("indeks-daftar-lain", program);
    assert_eq!(k, "1\n");
    if let Some(k) = jalankan_asan("indeks-daftar-lain-asan", program) {
        assert_eq!(k, "1\n");
    }
}

#[test]
fn nilai_tugas_dievaluasi_sebelum_indeks_tujuan() {
    let program = r#"
fungsi indeks(d: Daftar<bil>) -> bil { kembali d.panjang() }
fungsi utama() -> bil {
    ub a: [bil; 2] = [1, 2];
    ub d: Daftar<bil> = daftar();
    d.tambah(3);
    a[indeks(d)] = d.panjang();
    cetak(a[1]);
    kembali 0
}
"#;
    let k = sama_di_dua_mesin("nilai-sebelum-indeks", program);
    assert_eq!(k, "1\n");
    if let Some(k) = jalankan_asan("nilai-sebelum-indeks-asan", program) {
        assert_eq!(k, "1\n");
    }
}

#[test]
fn ir_menandai_fungsi_untuk_address_sanitizer() {
    let d = dir("ir-asan");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi utama() -> bil {
    ub d: Daftar<bil> = daftar();
    d.tambah(7);
    kembali d[0]
}
"#,
    );
    let ir = jalankan(&["ir", f.to_str().unwrap()]);
    assert!(ir.status.success(), "{}", teks_galat(&ir));
    let teks = teks_keluar(&ir);
    assert!(teks.contains("sanitize_address"), "{teks}");

    let Some(clang) = clang() else {
        return;
    };
    let ll = d.join("program.ll");
    fs::write(&ll, &ir.stdout).unwrap();
    let hasil = d.join("asan.ll");
    let c = Command::new(clang)
        .args(["-fsanitize=address", "-O0", "-S", "-emit-llvm", "-o"])
        .arg(&hasil)
        .arg(&ll)
        .output()
        .unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    let instrumentasi = fs::read_to_string(&hasil).unwrap();
    assert!(
        instrumentasi.contains("call void @__asan_report_load")
            || instrumentasi.contains("call void @__asan_load"),
        "akses heap tidak diinstrumentasi: {}",
        instrumentasi
    );
}

#[test]
fn bukti_irisan_yang_memendek_tidak_lolos() {
    if !ada_z3() {
        return;
    }
    let d = dir("bukti-irisan-memendek");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi utama() -> bil {
    ub a: [bil] = [10, 20];
    ub jalan: log = benar;
    selama jalan {
        a = [10];
        jalan = salah;
    }
    kembali a[1]
}
"#,
    );
    let o = jalankan(&["bukti", f.to_str().unwrap()]);
    assert!(!o.status.success());
    let gabung = format!("{}{}", teks_keluar(&o), teks_galat(&o));
    assert!(gabung.contains("indeks dalam batas"), "{gabung}");

    let biner = d.join("program");
    let o = jalankan(&[
        "bangun",
        f.to_str().unwrap(),
        "-o",
        biner.to_str().unwrap(),
        "--bebas-jaga",
    ]);
    assert!(!o.status.success());
    assert!(
        teks_galat(&o).contains("--bebas-jaga"),
        "{}",
        teks_galat(&o)
    );
}

#[test]
fn pinjaman_tempat_dengan_indeks_yang_memindahkan_ditolak() {
    periksa(
        "pinjam-indeks-memindahkan",
        r#"
fungsi indeks(d: Daftar<bil>) -> bil { kembali 0 }
fungsi pakai(x: &bil) { cetak("ok") }
fungsi utama() -> bil {
    ub a: Daftar<bil> = daftar();
    a.tambah(1);
    pakai(&a[indeks(a)]);
    kembali 0
}
"#,
        "E0320",
    );
}

#[test]
fn bukti_penulisan_elemen_mempertahankan_panjang_irisan() {
    if !ada_z3() {
        return;
    }
    let d = dir("bukti-elemen-panjang");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi utama() -> bil {
    ub a: [bil] = [10, 20];
    untuk i dalam 0..1 { a[0] = 5 }
    tegas a[1] == a[1]
    kembali 0
}
"#,
    );
    let o = jalankan(&["bukti", f.to_str().unwrap()]);
    assert!(o.status.success(), "{}", teks_galat(&o));
}

#[test]
fn bukti_bayang_lokal_memulihkan_irisan_luar() {
    if !ada_z3() {
        return;
    }
    let d = dir("bukti-bayang-irisan");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi utama() -> bil {
    ub a: [bil] = [10, 20];
    {
        a = [1, 2];
        ub a: [bil; 1] = [0];
        a[0] = 3;
    }
    tegas a[0] == 1
    tegas a[1] == 2
    kembali 0
}
"#,
    );
    let o = jalankan(&["bukti", f.to_str().unwrap()]);
    assert!(o.status.success(), "{}", teks_galat(&o));
}

#[test]
fn bukti_larik_tetap_tidak_kehilangan_panjang_setelah_loop() {
    if !ada_z3() {
        return;
    }
    let d = dir("bukti-larik-panjang");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi utama() -> bil {
    ub a: [bil; 1] = [0];
    selama a[0] == 0 { a[0] = 1 }
    tegas a[0] == a[0]
    kembali 0
}
"#,
    );
    let o = jalankan(&["bukti", f.to_str().unwrap()]);
    assert!(o.status.success(), "{}", teks_galat(&o));
}

#[test]
fn bukti_loop_rentang_mempertahankan_binding_luar() {
    if !ada_z3() {
        return;
    }
    let d = dir("bukti-bayang-rentang");
    let f = tulis(
        &d,
        "program.ns",
        r#"
fungsi utama() -> bil {
    ub i: bil = 7;
    untuk i dalam 0..2 { cetak(i) }
    tegas i == 7
    kembali 0
}
"#,
    );
    let o = jalankan(&["bukti", f.to_str().unwrap()]);
    assert!(o.status.success(), "{}", teks_galat(&o));
}

#[test]
fn variabel_bayangan_di_badan_iterasi_boleh_ditulis() {
    let k = sama_di_dua_mesin(
        "bayang-iterasi",
        r#"
fungsi utama() -> bil {
    ub d: Daftar<bil> = daftar();
    d.tambah(5);
    untuk x dalam d {
        ub d: bil = 0;
        d = x;
        cetak(d);
    }
    kembali 0
}
"#,
    );
    assert_eq!(k, "5\n");
}
