"""Uji otomatis kompiler Nusa.

Jalankan:  python3 -m unittest discover -s uji -p "uji_*.py" -v
"""
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NUSA = [sys.executable, os.path.join(ROOT, "bin", "nusa")]


def jalankan(*args):
    return subprocess.run(NUSA + list(args), capture_output=True, text=True, cwd=ROOT)


def tulis(tmp, nama, isi):
    path = os.path.join(tmp, nama)
    with open(path, "w", encoding="utf-8") as f:
        f.write(textwrap.dedent(isi))
    return path


def bangun(tmp, isi, nama="uji.ns", opsi=()):
    return jalankan("bangun", tulis(tmp, nama, isi), "-o", os.path.join(tmp, "prog"), *opsi)


def jalan(tmp, isi, nama="uji.ns", opsi=()):
    path = tulis(tmp, nama, isi)
    exe = os.path.join(tmp, "prog")
    build = jalankan("bangun", path, "-o", exe, *opsi)
    if build.returncode != 0:
        return build.returncode, "", build.stderr
    hasil = subprocess.run([exe], capture_output=True, text=True)
    return hasil.returncode, hasil.stdout, hasil.stderr


class UjiDasar(unittest.TestCase):
    def test_halo(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, out, _ = jalan(tmp, 'fungsi utama() { cetak("halo"); }')
            self.assertEqual(kode, 0)
            self.assertEqual(out, "halo\n")

    def test_aritmetika_dan_tipe(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, out, _ = jalan(tmp, """
                fungsi utama() {
                  cetak(1 + 2 * 3, 100 / 7, 100 % 7, 1 << 5, 12 & 10);
                  cetak(7 sebagai pecahan / 2.0, 3.99 sebagai bulat);
                }
            """)
            self.assertEqual(kode, 0)
            self.assertEqual(out, "7 14 2 32 8\n3.5 3\n")

    def test_logika_singkat_sirkuit(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, out, _ = jalan(tmp, """
                fungsi utama() {
                  ub n: bulat = 0;
                  jika n != 0 dan 100 / n > 2 { cetak("tak terduga"); }
                  cetak("aman", n == 0 atau 100 / n > 2);
                  cetak(bukan (n == 0), n == 0 dan benar);
                }
            """)
            self.assertEqual(kode, 0, out)
            self.assertEqual(out, "aman benar\nsalah benar\n")

    def test_rekursi(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, out, _ = jalan(tmp, """
                fungsi faktorial(n: bulat) -> bulat {
                  jika n <= 1 { kembali 1; }
                  kembali n * faktorial(n - 1);
                }
                fungsi utama() { cetak(faktorial(10)); }
            """)
            self.assertEqual(kode, 0)
            self.assertEqual(out, "3628800\n")

    def test_bentuk_pilihan_dan_cocok(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, out, _ = jalan(tmp, """
                bentuk Titik { x: bulat, y: bulat }
                pilihan Arah { Utara, Selatan }
                fungsi utama() {
                  tp t: Titik = Titik { x: 3, y: 4 };
                  ub n: bulat = 0;
                  cocok Arah.Utara {
                    kasus Utara { n = 1; }
                    kasus Selatan { n = 2; }
                  }
                  cetak(t.x + t.y, n);
                }
            """)
            self.assertEqual(kode, 0)
            self.assertEqual(out, "7 1\n")

    def test_deret_dan_putaran(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, out, _ = jalan(tmp, """
                fungsi jumlah(d: deret<bulat, 4>) -> bulat {
                  ub s: bulat = 0;
                  untuk i dalam 0..4 { s += d[i]; }
                  kembali s;
                }
                fungsi utama() {
                  cetak(jumlah([1, 2, 3, 4]), panjang([9, 9]));
                }
            """)
            self.assertEqual(kode, 0)
            self.assertEqual(out, "10 2\n")

    def test_teks_dan_panjang(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, out, _ = jalan(tmp, """
                fungsi utama() {
                  tp s: teks = "nusantara";
                  cetak(panjang(s), s == "nusantara", s == "lain");
                }
            """)
            self.assertEqual(kode, 0)
            self.assertEqual(out, "9 benar salah\n")

    def test_agregat_dan_semantik_nilai(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, out, _ = jalan(tmp, """
                bentuk Kotak { isi: deret<bulat, 3>, nama: teks }
                fungsi buat() -> deret<bulat, 3> { kembali [7, 8, 9]; }
                fungsi utama() {
                  tp k: Kotak = Kotak { isi: [1, 2, 3], nama: "kotak" };
                  ub d: deret<bulat, 3> = buat();
                  d[1] = 42;
                  ub k2: Kotak = k;
                  k2.isi[1] = 99;
                  cetak(k.nama, d[1], k.isi[1], k2.isi[1]);
                }
            """)
            self.assertEqual(kode, 0, out)
            self.assertEqual(out, "kotak 42 2 99\n")

    def test_geser_dan_konversi_dicek(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, _, err = jalan(tmp, """
                fungsi utama() { ub a: bulat = 1; ub b: bulat = 64; cetak(a << b); }
            """)
            self.assertNotEqual(kode, 0)
            self.assertIn("geser", err)
        with tempfile.TemporaryDirectory() as tmp:
            kode, _, err = jalan(tmp, """
                fungsi utama() { ub f: pecahan = 1.0e300; cetak(f sebagai bulat); }
            """)
            self.assertNotEqual(kode, 0)
            self.assertIn("di luar rentang", err)

    def test_tingkat_optimasi(self):
        for opt in ("-O0", "-O2", "-O3"):
            with tempfile.TemporaryDirectory() as tmp:
                kode, out, _ = jalan(tmp, 'fungsi utama() { cetak(6 * 7); }', opsi=(opt,))
                self.assertEqual(kode, 0)
                self.assertEqual(out, "42\n")

    def test_ukuran_biner_kecil(self):
        with tempfile.TemporaryDirectory() as tmp:
            build = bangun(tmp, 'fungsi utama() { cetak("x"); }')
            self.assertEqual(build.returncode, 0, build.stderr)
            ukuran = os.path.getsize(os.path.join(tmp, "prog"))
            self.assertLess(ukuran, 100_000)


class UjiKeamanan(unittest.TestCase):
    def test_indeks_di_luar_batas(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, _, err = jalan(tmp, """
                fungsi utama() {
                  tp d: deret<bulat, 3> = [1, 2, 3];
                  cetak(d[3]);
                }
            """)
            self.assertNotEqual(kode, 0)
            self.assertIn("di luar batas", err)

    def test_luapan(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, _, err = jalan(tmp, """
                fungsi utama() {
                  ub x: bulat8 = 127;
                  x = x + 1;
                  cetak(x);
                }
            """)
            self.assertNotEqual(kode, 0)
            self.assertIn("luapan", err)

    def test_bagi_nol(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, _, err = jalan(tmp, """
                fungsi utama() {
                  ub b: bulat = 0;
                  cetak(10 / b);
                }
            """)
            self.assertNotEqual(kode, 0)
            self.assertIn("nol", err)

    def test_takaman_perlu_izin(self):
        sumber = 'fungsi utama() { takaman { cetak("x"); } }'
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, "t.ns", sumber)
            tolak = jalankan("cek", path)
            self.assertEqual(tolak.returncode, 1)
            self.assertIn("--boleh-takaman", tolak.stdout + tolak.stderr)
            izin = jalankan("cek", path, "--boleh-takaman")
            self.assertEqual(izin.returncode, 0)

    def test_takaman_membungkus_tanpa_panik(self):
        with tempfile.TemporaryDirectory() as tmp:
            kode, out, _ = jalan(tmp, """
                fungsi utama() {
                  ub x: bulat8 = 127;
                  takaman { x = x + 1; }
                  cetak(x);
                }
            """, opsi=("--boleh-takaman",))
            self.assertEqual(kode, 0)
            self.assertEqual(out, "-128\n")


class UjiPenafsir(unittest.TestCase):
    def tafsir(self, sumber, opsi=(), nama="t.ns"):
        with tempfile.TemporaryDirectory() as tmp:
            hasil = jalankan("jalan", tulis(tmp, nama, sumber), *opsi)
            return hasil.returncode, hasil.stdout, hasil.stderr

    def banding(self, sumber, nama="t.ns", opsi=()):
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, nama, sumber)
            tafsir = jalankan("jalan", path, *opsi)
            exe = os.path.join(tmp, "prog")
            build = jalankan("bangun", path, "-o", exe, *opsi)
            self.assertEqual(build.returncode, 0, build.stderr)
            native = subprocess.run([exe], capture_output=True, text=True)
            return tafsir, native

    @staticmethod
    def kode_keluar(kode):
        return 128 - kode if kode < 0 else kode

    def test_mode_penafsir_tanpa_penaut_c(self):
        kode, out, err = self.tafsir('fungsi utama() { cetak("halo"); }',
                                     opsi=("--cc", "/bin/false"))
        self.assertEqual(kode, 0, err)
        self.assertEqual(out, "halo\n")
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, "t.ns", 'fungsi utama() { cetak("halo"); }')
            native = jalankan("jalan", path, "--native", "--cc", "/bin/false")
            self.assertNotEqual(native.returncode, 0)

    def test_alias_perintah(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, "t.ns", "fungsi utama() { cetak(1 + 1); }")
            for mode in ("jalan", "jalankan", "interp", "interpretasi"):
                hasil = jalankan(mode, path)
                self.assertEqual(hasil.returncode, 0, hasil.stderr)
                self.assertEqual(hasil.stdout, "2\n")

    def test_semua_contoh_sama_di_kedua_mode(self):
        for nama in ("halo", "aman", "fitur", "kritis", "kontrak", "panik"):
            with self.subTest(contoh=nama):
                path = os.path.join(ROOT, "contoh", nama + ".ns")
                tafsir = jalankan("jalan", path)
                with tempfile.TemporaryDirectory() as tmp:
                    exe = os.path.join(tmp, "prog")
                    build = jalankan("bangun", path, "-o", exe)
                    self.assertEqual(build.returncode, 0, build.stderr)
                    native = subprocess.run([exe], capture_output=True, text=True)
                self.assertEqual(tafsir.stdout, native.stdout, nama)
                self.assertEqual(self.kode_keluar(tafsir.returncode),
                                 self.kode_keluar(native.returncode), nama)
                self.assertEqual(tafsir.stderr.splitlines()[:1],
                                 native.stderr.splitlines()[:1], nama)

    def test_panik_sama_dengan_kompilasi(self):
        sumber = """
            fungsi utama() {
              tp d: deret<bulat, 2> = [1, 2];
              cetak("mulai");
              cetak(d[5]);
            }
        """
        tafsir, native = self.banding(sumber)
        self.assertIn("di luar batas", tafsir.stderr)
        self.assertIn("di luar batas", native.stderr)
        self.assertEqual(tafsir.stderr, native.stderr)
        self.assertEqual(tafsir.stdout, "mulai\n")
        self.assertEqual(self.kode_keluar(tafsir.returncode), 134)
        self.assertEqual(self.kode_keluar(native.returncode), 134)

    def test_luapan_dan_bagi_nol_sama(self):
        for sumber, frasa in (
                ("""
                 fungsi utama() { ub x: bulat8 = 127; x = x + 1; cetak(x); }
                 """, "luapan"),
                ("""
                 fungsi utama() { ub b: bulat = 0; cetak(7 / b); }
                 """, "pembagian dengan nol"),
                ("""
                 fungsi utama() { ub f: pecahan = 1.0e300; cetak(f sebagai bulat); }
                 """, "di luar rentang"),
        ):
            with self.subTest(frasa=frasa):
                tafsir, native = self.banding(sumber)
                self.assertIn(frasa, tafsir.stderr)
                self.assertEqual(tafsir.stderr, native.stderr)

    def test_semantik_nilai_deret_dan_bentuk(self):
        sumber = """
            bentuk Kotak { isi: deret<bulat, 2>, nama: teks }
            fungsi tambah_satu(d: deret<bulat, 2>) -> deret<bulat, 2> {
              kembali [d[0] + 1, d[1] + 1];
            }
            fungsi utama() {
              ub a: deret<bulat, 2> = [1, 2];
              ub b: deret<bulat, 2> = a;
              b[0] = 9;
              tp k: Kotak = Kotak { isi: a, nama: "k" };
              ub k2: Kotak = k;
              k2.isi[1] = 7;
              cetak(a[0], b[0], k.isi[1], k2.isi[1]);
              ub c: deret<bulat, 2> = tambah_satu(a);
              cetak(c[0], c[1], a[0]);
            }
        """
        kode, out, err = self.tafsir(sumber)
        self.assertEqual(kode, 0, err)
        self.assertEqual(out, "1 9 2 7\n2 3 1\n")
        tafsir, native = self.banding(sumber)
        self.assertEqual(tafsir.stdout, native.stdout)

    def test_takaman_dan_format_keluaran(self):
        sumber = """
            pilihan Warna { Merah, Hijau }
            fungsi utama() {
              ub x: bulat8 = 127;
              takaman { x = x + 1; }
              cetak(x, -0.5, 1.0e300, benar, Warna.Hijau, "teks");
              ub y: bulat8 = 100;
              takaman { y = y << 2; }
              cetak(y);
            }
        """
        kode, out, err = self.tafsir(sumber, opsi=("--boleh-takaman",))
        self.assertEqual(kode, 0, err)
        self.assertEqual(out, "-128 -0.5 1e+300 benar Hijau teks\n-112\n")
        tafsir, native = self.banding(sumber, opsi=("--boleh-takaman",))
        self.assertEqual(tafsir.stdout, native.stdout)

    def test_torture_sama_di_kedua_mode(self):
        sumber = """
            bentuk Pasangan { a: bulat, b: cacah32 }
            pilihan Status { Siap, Jalan, Selesai }
            fungsi bagi(a: bulat, b: bulat) -> bulat { kembali a / b; }
            fungsi sisa(a: bulat, b: bulat) -> bulat { kembali a % b; }
            fungsi klasifikasi(n: bulat) -> teks {
              jika n < 0 { kembali "negatif"; }
              lainnya jika n == 0 { kembali "nol"; }
              lainnya jika n < 10 { kembali "kecil"; }
              kembali "besar";
            }
            fungsi utama() {
              cetak("bagi", bagi(-7, 2), bagi(7, -2), sisa(-7, 2), sisa(7, -2));
              cetak("klas", klasifikasi(-3), klasifikasi(0), klasifikasi(5), klasifikasi(50));
              ub total: bulat = 0;
              untuk i dari 1 sampai 4 { total += i; }
              cetak("inklusif", total);
              ub j: bulat = 0;
              ulang {
                j += 1;
                jika j == 3 { lanjut; }
                jika j > 5 { henti; }
                total += j;
              }
              cetak("ulang", total, j);
              ub p: Pasangan = Pasangan { a: -5, b: 4000000000 };
              cetak("pasangan", p.a, p.b, p.b / 3);
              ub st: Status = Status.Jalan;
              cetak("status", st);
              ub u: cacah64 = 18446744073709551615;
              cetak("cacah64", u, u / 2);
              ub kecil: bulat8 = -128;
              ub lebar: bulat = kecil sebagai bulat;
              ub balik: bulat8 = lebar sebagai bulat8;
              cetak("konversi", lebar, balik);
              ub f: pecahan32 = 1.1;
              ub g: pecahan32 = f * 3.0;
              cetak("f32", f, g);
              cetak("bit", 12 & 10, 12 | 10, 12 ^ 10, ~12, 1 << 40, (-8) >> 1);
              cetak("banding", "abc" < "abd", "abc" == "abc", 3 <= 3, 3 < 3);
              periksa total > 0, "total harus positif";
              cetak("ok");
            }
        """
        harapan = (
            "bagi -3 -3 -1 1\n"
            "klas negatif nol kecil besar\n"
            "inklusif 10\n"
            "ulang 22 6\n"
            "pasangan -5 4000000000 1333333333\n"
            "status Jalan\n"
            "cacah64 18446744073709551615 9223372036854775807\n"
            "konversi -128 -128\n"
            "f32 1.1 3.3\n"
            "bit 8 14 6 -13 1099511627776 -4\n"
            "banding benar benar benar salah\n"
            "ok\n"
        )
        kode, out, err = self.tafsir(sumber)
        self.assertEqual(kode, 0, err)
        self.assertEqual(out, harapan)
        tafsir, native = self.banding(sumber)
        self.assertEqual(tafsir.stdout, harapan)
        self.assertEqual(native.stdout, harapan)

    def test_literal_negatif_ekstrem(self):
        sumber = """
            fungsi utama() {
              tp a: bulat8 = -128;
              tp b: bulat = -9223372036854775808;
              cetak(a, b);
            }
        """
        kode, out, err = self.tafsir(sumber)
        self.assertEqual(kode, 0, err)
        self.assertEqual(out, "-128 -9223372036854775808\n")
        tafsir, native = self.banding(sumber)
        self.assertEqual(tafsir.stdout, native.stdout)

    def test_rekursi_dan_kode_keluar(self):
        sumber = """
            fungsi fib(n: bulat) -> bulat {
              jika n < 2 { kembali n; }
              kembali fib(n - 1) + fib(n - 2);
            }
            fungsi utama() -> bulat {
              cetak(fib(15));
              kembali 7;
            }
        """
        kode, out, _ = self.tafsir(sumber)
        self.assertEqual(kode, 7)
        self.assertEqual(out, "610\n")
        tafsir, native = self.banding(sumber)
        self.assertEqual(tafsir.stdout, native.stdout)
        self.assertEqual(self.kode_keluar(tafsir.returncode),
                         self.kode_keluar(native.returncode))

    def test_putaran_dan_cocok(self):
        sumber = """
            pilihan Arah { Utara, Selatan, Timur }
            fungsi nama(a: Arah) -> teks {
              cocok a {
                kasus Utara { kembali "U"; }
                kasus Selatan { kembali "S"; }
                kasus Timur { kembali "T"; }
              }
            }
            fungsi utama() {
              ub total: bulat = 0;
              ub i: bulat = 0;
              selama i < 5 {
                untuk j dalam 0..i { total += j; }
                i += 1;
              }
              cetak(total, nama(Arah.Timur));
            }
        """
        kode, out, err = self.tafsir(sumber)
        self.assertEqual(kode, 0, err)
        self.assertEqual(out, "10 T\n")
        tafsir, native = self.banding(sumber)
        self.assertEqual(tafsir.stdout, native.stdout)


class UjiPenolakan(unittest.TestCase):
    def cek_galat(self, sumber, frasa):
        with tempfile.TemporaryDirectory() as tmp:
            hasil = jalankan("cek", tulis(tmp, "g.ns", sumber))
            self.assertEqual(hasil.returncode, 1, hasil.stdout)
            self.assertIn(frasa, hasil.stdout + hasil.stderr)

    def test_tetap_tidak_bisa_diubah(self):
        self.cek_galat('fungsi utama() { tp x: bulat = 1; x = 2; }', "nilai tetap")

    def test_tipe_tidak_cocok(self):
        self.cek_galat('fungsi utama() { ub y: cacah = 1 sebagai bulat; }', "tipe tidak cocok")

    def test_cocok_tidak_lengkap(self):
        self.cek_galat("""
            pilihan W { A, B }
            fungsi utama() { cocok W.A { kasus A { } } }
        """, "cocok tidak lengkap")

    def test_harus_kembali(self):
        self.cek_galat("""
            fungsi f(x: bulat) -> bulat { jika x > 0 { kembali 1; } }
            fungsi utama() { cetak(f(1)); }
        """, "harus mengembalikan nilai")

    def test_nama_tak_dikenal(self):
        self.cek_galat('fungsi utama() { cetak(tidak_ada + 1); }', "tidak dikenal")

    def test_aritmatika_tipe_campur(self):
        self.cek_galat("""
            fungsi utama() { ub a: bulat32 = 1; ub b: bulat = 2; cetak(a + b); }
        """, "antar tipe berbeda")

    def test_literal_di_luar_rentang(self):
        self.cek_galat('fungsi utama() { tp x: bulat8 = 300; }', "di luar rentang")

    def test_negasi_cacah_ditolak(self):
        self.cek_galat("""
            fungsi utama() { ub a: cacah = 5; cetak(-a); }
        """, "negasi")

    def test_tanpa_utama(self):
        self.cek_galat("fungsi lain() { }", "utama")

    def test_jumlah_argumen(self):
        self.cek_galat("""
            fungsi f(a: bulat) -> bulat { kembali a; }
            fungsi utama() { cetak(f(1, 2)); }
        """, "argumen")


class UjiAlat(unittest.TestCase):
    def test_ir_dan_asm(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, "x.ns", 'fungsi utama() { cetak("x"); }')
            ir = jalankan("ir", path)
            self.assertEqual(ir.returncode, 0)
            self.assertIn("define", ir.stdout)
            asm = jalankan("asm", path)
            self.assertEqual(asm.returncode, 0)
            self.assertIn("main", asm.stdout)

    def test_simpan_objek_dan_ir(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, "x.ns", 'fungsi utama() { cetak("x"); }')
            obj = os.path.join(tmp, "x.o")
            ll = os.path.join(tmp, "x.ll")
            hasil = jalankan("bangun", path, "-o", os.path.join(tmp, "x"),
                         "--obj", obj, "--ir", ll)
            self.assertEqual(hasil.returncode, 0, hasil.stderr)
            self.assertTrue(os.path.getsize(obj) > 0)
            with open(ll, encoding="utf-8") as f:
                self.assertIn("define", f.read())

    def test_strip(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, "x.ns", 'fungsi utama() { cetak("x"); }')
            hasil = jalankan("bangun", path, "-o", os.path.join(tmp, "x"), "-s")
            self.assertEqual(hasil.returncode, 0, hasil.stderr)


class UjiBukti(unittest.TestCase):
    def test_kontrak_terbukti(self):
        hasil = jalankan("bukti", os.path.join(ROOT, "contoh", "kontrak.ns"))
        self.assertEqual(hasil.returncode, 0, hasil.stdout + hasil.stderr)
        self.assertIn("hitung_naik", hasil.stdout)
        self.assertIn("0 terbantah", hasil.stdout)

    def test_kritis_terbukti(self):
        hasil = jalankan("bukti", os.path.join(ROOT, "contoh", "kritis.ns"))
        self.assertEqual(hasil.returncode, 0, hasil.stdout + hasil.stderr)

    def test_pascakondisi_salah_terbantah(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, "b.ns", """
                murni fungsi f(a: bulat) -> bulat
                  syarat a > 0
                  jamin hasil > a + 1
                { kembali a + 1; }
                fungsi utama() { cetak(f(1)); }
            """)
            hasil = jalankan("bukti", path)
            self.assertEqual(hasil.returncode, 3, hasil.stdout)
            self.assertIn("TERBANTAH", hasil.stdout)

    def test_luapan_negasi_terdeteksi(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, "b.ns", """
                murni fungsi mutlak(x: bulat) -> bulat
                  jamin hasil >= 0
                { jika x < 0 { kembali 0 - x; } kembali x; }
                fungsi utama() { cetak(mutlak(1)); }
            """)
            hasil = jalankan("bukti", path)
            self.assertEqual(hasil.returncode, 3, hasil.stdout)
            self.assertIn("luapan", hasil.stdout)

    def test_periksa_terbantah_dengan_contoh(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, "b.ns", """
                fungsi f(x: bulat) -> bulat {
                  periksa x >= 0, "x harus positif";
                  kembali x;
                }
                fungsi utama() { cetak(f(1)); }
            """)
            hasil = jalankan("bukti", path)
            self.assertEqual(hasil.returncode, 3, hasil.stdout)
            self.assertIn("contoh penyangkal", hasil.stdout)

    def test_putaran_tanpa_invarian_tidak_pasti(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = tulis(tmp, "b.ns", """
                murni fungsi f(n: bulat) -> bulat
                  syarat n >= 0
                  jamin hasil >= 0
                { ub i: bulat = 0; selama i < n { i += 1; } kembali i; }
                fungsi utama() { cetak(f(1)); }
            """)
            hasil = jalankan("bukti", path)
            self.assertEqual(hasil.returncode, 2, hasil.stdout)
            self.assertIn("invarian", hasil.stdout)

    def test_dump_smt(self):
        with tempfile.TemporaryDirectory() as tmp:
            smt = os.path.join(tmp, "smt")
            hasil = jalankan("bukti", os.path.join(ROOT, "contoh", "kontrak.ns"), "--smt", smt)
            self.assertEqual(hasil.returncode, 0, hasil.stdout)
            berkas = os.listdir(smt)
            self.assertTrue(berkas)
            with open(os.path.join(smt, berkas[0]), encoding="utf-8") as f:
                isi = f.read()
            self.assertIn("(check-sat)", isi)
            self.assertIn("declare-fun", isi)


if __name__ == "__main__":
    unittest.main(verbosity=2)
