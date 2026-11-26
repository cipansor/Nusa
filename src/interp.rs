//! Interpreter langsung (tree-walking) untuk Nusa.
//!
//! Mode ini menjalankan AST tanpa `clang`/LLVM. Semantik nilai, pemeriksaan
//! runtime, format keluaran, dan pesan panic dibuat sama dengan hasil kompilasi.

use crate::ast::{
    cetak_ekspr, BinOp, Expr, ExprKind, Fungsi, Iterasi, Program, Stmt, TugasOp, UnOp,
};
use std::collections::HashMap;
use std::rc::Rc;

const KEDALAMAN_MAKS: usize = 2000;

#[derive(Clone, Copy, Debug)]
pub struct Opsi {
    /// Periksa luapan aritmetika `bil`.
    pub aman: bool,
    /// Periksa `syarat`/`jamin` saat berjalan.
    pub kontrak: bool,
}

impl Default for Opsi {
    fn default() -> Self {
        Opsi {
            aman: true,
            kontrak: true,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Nilai {
    Bil(i64),
    Des(f64),
    Log(bool),
    Teks(Rc<str>),
    Daftar(Rc<Vec<Nilai>>),
    Kosong,
}

impl Nilai {
    fn sebagai_bil(&self) -> Hasil<i64> {
        match self {
            Nilai::Bil(n) => Ok(*n),
            _ => Err(Kesalahan::internal("nilai bukan `bil`")),
        }
    }

    fn sebagai_log(&self) -> Hasil<bool> {
        match self {
            Nilai::Log(b) => Ok(*b),
            _ => Err(Kesalahan::internal("nilai bukan `log`")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Kesalahan {
    pub pesan: String,
    pub internal: bool,
}

impl Kesalahan {
    fn panik(pesan: impl Into<String>) -> Self {
        Kesalahan {
            pesan: pesan.into(),
            internal: false,
        }
    }

    fn internal(pesan: impl Into<String>) -> Self {
        Kesalahan {
            pesan: pesan.into(),
            internal: true,
        }
    }
}

type Hasil<T> = Result<T, Kesalahan>;

fn panik<T>(pesan: impl Into<String>) -> Hasil<T> {
    Err(Kesalahan::panik(pesan))
}

fn internal<T>(pesan: impl Into<String>) -> Hasil<T> {
    Err(Kesalahan::internal(pesan))
}

#[derive(Clone, Debug)]
struct Ikatan {
    nilai: Nilai,
    ubah: bool,
}

#[derive(Debug)]
enum Aliran {
    Normal,
    Kembalikan(Nilai),
    Hentikan,
    Lanjutkan,
}

pub struct HasilJalan {
    pub keluaran: String,
    pub kesalahan: Option<Kesalahan>,
}

pub fn jalankan(program: &Program, opsi: Opsi) -> HasilJalan {
    let fungsi = program
        .fungsi
        .iter()
        .map(|f| (f.nama.as_str(), f))
        .collect::<HashMap<_, _>>();
    let utama = match fungsi.get("utama") {
        Some(f) => *f,
        None => {
            return HasilJalan {
                keluaran: String::new(),
                kesalahan: Some(Kesalahan::internal("fungsi `utama` tidak ditemukan")),
            }
        }
    };
    let mut i = Interp {
        fungsi,
        lingkup: Vec::new(),
        keluaran: String::new(),
        opsi,
        kedalaman: 0,
    };
    let hasil = i.panggil(utama, Vec::new());
    HasilJalan {
        keluaran: i.keluaran,
        kesalahan: hasil.err(),
    }
}

struct Interp<'a> {
    fungsi: HashMap<&'a str, &'a Fungsi>,
    lingkup: Vec<HashMap<String, Ikatan>>,
    keluaran: String,
    opsi: Opsi,
    kedalaman: usize,
}

impl Interp<'_> {
    fn cari(&self, nama: &str) -> Option<&Ikatan> {
        self.lingkup.iter().rev().find_map(|l| l.get(nama))
    }

    fn cari_mut(&mut self, nama: &str) -> Option<&mut Ikatan> {
        self.lingkup.iter_mut().rev().find_map(|l| l.get_mut(nama))
    }

    fn panggil(&mut self, f: &Fungsi, args: Vec<Nilai>) -> Hasil<Nilai> {
        if self.kedalaman >= KEDALAMAN_MAKS {
            return panik(format!(
                "batas kedalaman pemanggilan ({}) terlampaui",
                KEDALAMAN_MAKS
            ));
        }
        if args.len() != f.params.len() {
            return internal(format!(
                "fungsi `{}` menerima {} argumen, diberikan {}",
                f.nama,
                f.params.len(),
                args.len()
            ));
        }
        self.kedalaman += 1;
        let simpan = std::mem::take(&mut self.lingkup);
        self.lingkup = vec![HashMap::new()];
        for (p, v) in f.params.iter().zip(args) {
            self.lingkup[0].insert(
                p.nama.clone(),
                Ikatan {
                    nilai: v,
                    ubah: true,
                },
            );
        }

        let hasil = self.jalankan_fungsi(f);
        self.lingkup = simpan;
        self.kedalaman -= 1;
        hasil
    }

    fn jalankan_fungsi(&mut self, f: &Fungsi) -> Hasil<Nilai> {
        if self.opsi.kontrak {
            for s in &f.syarat {
                if !self.evaluasi(s)?.sebagai_log()? {
                    return panik(format!(
                        "syarat fungsi `{}` gagal: {}",
                        f.nama,
                        cetak_ekspr(s)
                    ));
                }
            }
        }

        let aliran = self.blok(&f.tubuh)?;
        let nilai = match aliran {
            Aliran::Kembalikan(v) => v,
            Aliran::Normal => {
                if f.hasil != crate::ast::Tipe::Kosong {
                    return internal(format!("fungsi `{}` selesai tanpa `kembalikan`", f.nama));
                }
                Nilai::Kosong
            }
            _ => {
                return internal(format!(
                    "aliran `hentikan`/`lanjutkan` keluar dari fungsi `{}`",
                    f.nama
                ))
            }
        };

        if self.opsi.kontrak && !f.jamin.is_empty() {
            self.lingkup[0].insert(
                "hasil".into(),
                Ikatan {
                    nilai: nilai.clone(),
                    ubah: false,
                },
            );
            let mut galat = None;
            for j in &f.jamin {
                match self.evaluasi(j).and_then(|v| v.sebagai_log()) {
                    Ok(true) => {}
                    Ok(false) => {
                        galat = Some(Kesalahan::panik(format!(
                            "jamin fungsi `{}` gagal: {}",
                            f.nama,
                            cetak_ekspr(j)
                        )));
                        break;
                    }
                    Err(e) => {
                        galat = Some(e);
                        break;
                    }
                }
            }
            self.lingkup[0].remove("hasil");
            if let Some(e) = galat {
                return Err(e);
            }
        }
        Ok(nilai)
    }

    fn blok(&mut self, stmts: &[Stmt]) -> Hasil<Aliran> {
        self.lingkup.push(HashMap::new());
        let hasil = self.blok_dalam(stmts);
        self.lingkup.pop();
        hasil
    }

    fn blok_dalam(&mut self, stmts: &[Stmt]) -> Hasil<Aliran> {
        for s in stmts {
            match self.pernyataan(s)? {
                Aliran::Normal => {}
                lain => return Ok(lain),
            }
        }
        Ok(Aliran::Normal)
    }

    fn pernyataan(&mut self, s: &Stmt) -> Hasil<Aliran> {
        match s {
            Stmt::Tetap { nama, nilai, .. } => {
                let v = self.evaluasi(nilai)?;
                self.deklarasikan(nama, v, false)?;
                Ok(Aliran::Normal)
            }
            Stmt::Var { nama, nilai, .. } => {
                let v = self.evaluasi(nilai)?;
                self.deklarasikan(nama, v, true)?;
                Ok(Aliran::Normal)
            }
            Stmt::Tugas {
                target, op, nilai, ..
            } => {
                self.tugas(target, *op, nilai)?;
                Ok(Aliran::Normal)
            }
            Stmt::Ekspresi(e) => {
                self.evaluasi(e)?;
                Ok(Aliran::Normal)
            }
            Stmt::Jika {
                kondisi,
                maka,
                lainnya,
                ..
            } => {
                if self.evaluasi(kondisi)?.sebagai_log()? {
                    self.blok(maka)
                } else if let Some(l) = lainnya {
                    self.blok(l)
                } else {
                    Ok(Aliran::Normal)
                }
            }
            Stmt::Selama { kondisi, tubuh, .. } => {
                while self.evaluasi(kondisi)?.sebagai_log()? {
                    match self.blok(tubuh)? {
                        Aliran::Kembalikan(v) => return Ok(Aliran::Kembalikan(v)),
                        Aliran::Hentikan => break,
                        Aliran::Lanjutkan | Aliran::Normal => {}
                    }
                }
                Ok(Aliran::Normal)
            }
            Stmt::Untuk {
                nama,
                iterasi,
                tubuh,
                ..
            } => self.untuk(nama, iterasi, tubuh),
            Stmt::Kembalikan { nilai, .. } => {
                let v = match nilai {
                    Some(e) => self.evaluasi(e)?,
                    None => Nilai::Kosong,
                };
                Ok(Aliran::Kembalikan(v))
            }
            Stmt::Tegaskan { kondisi, .. } => {
                if self.opsi.kontrak && !self.evaluasi(kondisi)?.sebagai_log()? {
                    return panik(format!("tegasan gagal: {}", cetak_ekspr(kondisi)));
                }
                Ok(Aliran::Normal)
            }
            Stmt::Hentikan { .. } => Ok(Aliran::Hentikan),
            Stmt::Lanjutkan { .. } => Ok(Aliran::Lanjutkan),
        }
    }

    fn deklarasikan(&mut self, nama: &str, nilai: Nilai, ubah: bool) -> Hasil<()> {
        let l = match self.lingkup.last_mut() {
            Some(l) => l,
            None => return internal("lingkup kosong saat deklarasi"),
        };
        if l.contains_key(nama) {
            return internal(format!("variabel `{}` dideklarasikan ulang", nama));
        }
        l.insert(nama.to_string(), Ikatan { nilai, ubah });
        Ok(())
    }

    fn tugas(&mut self, target: &Expr, op: TugasOp, nilai: &Expr) -> Hasil<()> {
        match &target.kind {
            ExprKind::Nama(nama) => {
                let lama = if op == TugasOp::Isi {
                    None
                } else {
                    Some(self.nilai_ikatan(nama, target)?.clone())
                };
                let kanan = self.evaluasi(nilai)?;
                let baru = match (op, lama) {
                    (TugasOp::Isi, _) => kanan,
                    (_, Some(lama)) => {
                        Self::hitung(tugas_ke_biner(op), lama, kanan, self.opsi.aman)?
                    }
                    _ => unreachable!(),
                };
                let ik = match self.cari_mut(nama) {
                    Some(i) => i,
                    None => return internal(format!("variabel `{}` tidak ditemukan", nama)),
                };
                if !ik.ubah {
                    return internal(format!("variabel `{}` tidak dapat diubah", nama));
                }
                ik.nilai = baru;
                Ok(())
            }
            ExprKind::Indeks(base, indeks) => {
                let nama = match &base.kind {
                    ExprKind::Nama(n) => n.clone(),
                    _ => return internal("target indeks bukan variabel"),
                };
                let daftar = self.nilai_daftar(&nama, target)?;
                let i = self.evaluasi(indeks)?.sebagai_bil()?;
                let lama = Self::ambil_elemen(&daftar, i)?.clone();
                let kanan = self.evaluasi(nilai)?;
                let baru = if op == TugasOp::Isi {
                    kanan
                } else {
                    Self::hitung(tugas_ke_biner(op), lama, kanan, self.opsi.aman)?
                };
                let ik = match self.cari_mut(&nama) {
                    Some(i) => i,
                    None => return internal(format!("variabel `{}` tidak ditemukan", nama)),
                };
                if !ik.ubah {
                    return internal(format!("variabel `{}` tidak dapat diubah", nama));
                }
                match &mut ik.nilai {
                    Nilai::Daftar(rc) => {
                        let v = Rc::make_mut(rc);
                        if i < 0 || i as usize >= v.len() {
                            return panik(format!(
                                "indeks di luar batas: indeks={} panjang={}",
                                i,
                                v.len()
                            ));
                        }
                        v[i as usize] = baru;
                        Ok(())
                    }
                    _ => internal("target indeks bukan daftar"),
                }
            }
            _ => internal("target penugasan tidak sah"),
        }
    }

    fn nilai_ikatan(&self, nama: &str, e: &Expr) -> Hasil<&Nilai> {
        match self.cari(nama) {
            Some(ik) => Ok(&ik.nilai),
            None => internal(format!(
                "variabel `{}` tidak ditemukan (span {})",
                nama, e.span.mulai
            )),
        }
    }

    fn nilai_daftar(&self, nama: &str, e: &Expr) -> Hasil<Rc<Vec<Nilai>>> {
        match self.nilai_ikatan(nama, e)? {
            Nilai::Daftar(rc) => Ok(Rc::clone(rc)),
            _ => internal(format!("variabel `{}` bukan daftar", nama)),
        }
    }

    fn ambil_elemen(daftar: &[Nilai], i: i64) -> Hasil<&Nilai> {
        if i < 0 || i as usize >= daftar.len() {
            return panik(format!(
                "indeks di luar batas: indeks={} panjang={}",
                i,
                daftar.len()
            ));
        }
        Ok(&daftar[i as usize])
    }

    fn untuk(&mut self, nama: &str, iterasi: &Iterasi, tubuh: &[Stmt]) -> Hasil<Aliran> {
        match iterasi {
            Iterasi::Rentang {
                mulai,
                akhir,
                inklusif,
            } => {
                let a = self.evaluasi(mulai)?.sebagai_bil()?;
                let b = self.evaluasi(akhir)?.sebagai_bil()?;
                self.lingkup.push(HashMap::new());
                self.lingkup.last_mut().unwrap().insert(
                    nama.to_string(),
                    Ikatan {
                        nilai: Nilai::Bil(a),
                        ubah: false,
                    },
                );
                let hasil = (|| {
                    let mut i = a;
                    loop {
                        let lanjut = if *inklusif { i <= b } else { i < b };
                        if !lanjut {
                            break;
                        }
                        if let Some(ik) = self.lingkup.last_mut().unwrap().get_mut(nama) {
                            ik.nilai = Nilai::Bil(i);
                        }
                        match self.blok(tubuh)? {
                            Aliran::Kembalikan(v) => return Ok(Aliran::Kembalikan(v)),
                            Aliran::Hentikan => break,
                            Aliran::Lanjutkan | Aliran::Normal => {}
                        }
                        i = if self.opsi.aman {
                            match i.checked_add(1) {
                                Some(n) => n,
                                None => return panik("luapan penghitung loop"),
                            }
                        } else {
                            i.wrapping_add(1)
                        };
                    }
                    Ok(Aliran::Normal)
                })();
                self.lingkup.pop();
                hasil
            }
            Iterasi::Setiap(e) => {
                let daftar = match self.evaluasi(e)? {
                    Nilai::Daftar(rc) => rc,
                    _ => return internal("`untuk ... dalam` pada bukan daftar"),
                };
                let panjang = daftar.len();
                self.lingkup.push(HashMap::new());
                self.lingkup.last_mut().unwrap().insert(
                    nama.to_string(),
                    Ikatan {
                        nilai: Nilai::Kosong,
                        ubah: false,
                    },
                );
                let hasil = (|| {
                    for i in 0..panjang {
                        if let Some(ik) = self.lingkup.last_mut().unwrap().get_mut(nama) {
                            ik.nilai = daftar[i].clone();
                        }
                        match self.blok(tubuh)? {
                            Aliran::Kembalikan(v) => return Ok(Aliran::Kembalikan(v)),
                            Aliran::Hentikan => break,
                            Aliran::Lanjutkan | Aliran::Normal => {}
                        }
                    }
                    Ok(Aliran::Normal)
                })();
                self.lingkup.pop();
                hasil
            }
        }
    }

    fn evaluasi(&mut self, e: &Expr) -> Hasil<Nilai> {
        match &e.kind {
            ExprKind::Bil(n) => Ok(Nilai::Bil(*n)),
            ExprKind::Des(n) => Ok(Nilai::Des(*n)),
            ExprKind::Teks(s) => Ok(Nilai::Teks(Rc::from(s.as_str()))),
            ExprKind::Log(b) => Ok(Nilai::Log(*b)),
            ExprKind::Nama(nama) => match self.cari(nama) {
                Some(ik) => Ok(ik.nilai.clone()),
                None => internal(format!("variabel `{}` tidak ditemukan", nama)),
            },
            ExprKind::Daftar(xs) => {
                let mut v = Vec::with_capacity(xs.len());
                for x in xs {
                    v.push(self.evaluasi(x)?);
                }
                Ok(Nilai::Daftar(Rc::new(v)))
            }
            ExprKind::Unary(op, x) => {
                let v = self.evaluasi(x)?;
                match op {
                    UnOp::Tidak => Ok(Nilai::Log(!v.sebagai_log()?)),
                    UnOp::Neg => match v {
                        Nilai::Bil(n) => {
                            if self.opsi.aman {
                                match n.checked_neg() {
                                    Some(r) => Ok(Nilai::Bil(r)),
                                    None => panik("luapan negasi"),
                                }
                            } else {
                                Ok(Nilai::Bil(n.wrapping_neg()))
                            }
                        }
                        Nilai::Des(d) => Ok(Nilai::Des(-d)),
                        _ => internal("negasi bukan angka"),
                    },
                }
            }
            ExprKind::Biner(op, a, b) => match op {
                BinOp::Dan => {
                    if !self.evaluasi(a)?.sebagai_log()? {
                        Ok(Nilai::Log(false))
                    } else {
                        Ok(Nilai::Log(self.evaluasi(b)?.sebagai_log()?))
                    }
                }
                BinOp::Atau => {
                    if self.evaluasi(a)?.sebagai_log()? {
                        Ok(Nilai::Log(true))
                    } else {
                        Ok(Nilai::Log(self.evaluasi(b)?.sebagai_log()?))
                    }
                }
                _ => {
                    let va = self.evaluasi(a)?;
                    let vb = self.evaluasi(b)?;
                    Self::hitung(*op, va, vb, self.opsi.aman)
                }
            },
            ExprKind::Panggil(nama, args) => self.panggil_ekspresi(nama, args),
            ExprKind::Metode(res, nama, args) => self.metode(res, nama, args),
            ExprKind::Indeks(base, indeks) => {
                let b = self.evaluasi(base)?;
                let i = self.evaluasi(indeks)?.sebagai_bil()?;
                match b {
                    Nilai::Daftar(rc) => Ok(Self::ambil_elemen(&rc, i)?.clone()),
                    _ => internal("indeks pada bukan daftar"),
                }
            }
        }
    }

    fn panggil_ekspresi(&mut self, nama: &str, args: &[Expr]) -> Hasil<Nilai> {
        match nama {
            "cetak" | "cetakln" => {
                for a in args {
                    let v = self.evaluasi(a)?;
                    let s = format_nilai(&v);
                    self.keluaran.push_str(&s);
                }
                if nama == "cetakln" {
                    self.keluaran.push('\n');
                }
                Ok(Nilai::Kosong)
            }
            "panjang" => {
                let v = self.evaluasi(&args[0])?;
                Ok(Nilai::Bil(match v {
                    Nilai::Teks(s) => s.len() as i64,
                    Nilai::Daftar(d) => d.len() as i64,
                    _ => return internal("`panjang` pada nilai yang salah"),
                }))
            }
            "mutlak" => {
                let v = self.evaluasi(&args[0])?;
                match v {
                    Nilai::Bil(n) => {
                        if n == i64::MIN {
                            panik("luapan pada mutlak")
                        } else {
                            Ok(Nilai::Bil(n.abs()))
                        }
                    }
                    Nilai::Des(d) => Ok(Nilai::Des(d.abs())),
                    _ => internal("`mutlak` pada nilai yang salah"),
                }
            }
            _ => {
                let f = match self.fungsi.get(nama) {
                    Some(f) => *f,
                    None => return internal(format!("fungsi `{}` tidak ditemukan", nama)),
                };
                let mut v = Vec::with_capacity(args.len());
                for a in args {
                    v.push(self.evaluasi(a)?);
                }
                self.panggil(f, v)
            }
        }
    }

    fn metode(&mut self, res: &Expr, nama: &str, args: &[Expr]) -> Hasil<Nilai> {
        match nama {
            "panjang" => {
                let v = self.evaluasi(res)?;
                Ok(Nilai::Bil(match v {
                    Nilai::Teks(s) => s.len() as i64,
                    Nilai::Daftar(d) => d.len() as i64,
                    _ => return internal("`panjang` pada nilai yang salah"),
                }))
            }
            "tambah" => {
                let penerima = match &res.kind {
                    ExprKind::Nama(n) => n.clone(),
                    _ => return internal("penerima `tambah` bukan variabel"),
                };
                let v = self.evaluasi(&args[0])?;
                let ik = match self.cari_mut(&penerima) {
                    Some(i) => i,
                    None => return internal(format!("variabel `{}` tidak ditemukan", penerima)),
                };
                if !ik.ubah {
                    return internal(format!("variabel `{}` tidak dapat diubah", penerima));
                }
                match &mut ik.nilai {
                    Nilai::Daftar(rc) => {
                        Rc::make_mut(rc).push(v);
                        Ok(Nilai::Kosong)
                    }
                    _ => internal("`tambah` pada bukan daftar"),
                }
            }
            _ => internal(format!("metode `{}` tidak dikenal", nama)),
        }
    }

    fn hitung(op: BinOp, a: Nilai, b: Nilai, aman: bool) -> Hasil<Nilai> {
        match op {
            BinOp::Tambah | BinOp::Kurang | BinOp::Kali | BinOp::Bagi | BinOp::Mod => {
                match (a, b) {
                    (Nilai::Bil(x), Nilai::Bil(y)) => {
                        let r = match op {
                            BinOp::Tambah => {
                                if aman {
                                    x.checked_add(y)
                                } else {
                                    Some(x.wrapping_add(y))
                                }
                            }
                            BinOp::Kurang => {
                                if aman {
                                    x.checked_sub(y)
                                } else {
                                    Some(x.wrapping_sub(y))
                                }
                            }
                            BinOp::Kali => {
                                if aman {
                                    x.checked_mul(y)
                                } else {
                                    Some(x.wrapping_mul(y))
                                }
                            }
                            BinOp::Bagi => return bagi_bil(x, y).map(Nilai::Bil),
                            BinOp::Mod => return mod_bil(x, y).map(Nilai::Bil),
                            _ => unreachable!(),
                        };
                        match r {
                            Some(n) => Ok(Nilai::Bil(n)),
                            None => panik(match op {
                                BinOp::Tambah => "luapan penjumlahan",
                                BinOp::Kurang => "luapan pengurangan",
                                _ => "luapan perkalian",
                            }),
                        }
                    }
                    (Nilai::Des(x), Nilai::Des(y)) => Ok(Nilai::Des(match op {
                        BinOp::Tambah => x + y,
                        BinOp::Kurang => x - y,
                        BinOp::Kali => x * y,
                        BinOp::Bagi => x / y,
                        _ => return internal("operator desimal tidak sah"),
                    })),
                    (Nilai::Teks(x), Nilai::Teks(y)) if op == BinOp::Tambah => {
                        let mut s = String::with_capacity(x.len() + y.len());
                        s.push_str(&x);
                        s.push_str(&y);
                        Ok(Nilai::Teks(Rc::from(s)))
                    }
                    _ => internal("operan aritmetika tidak cocok"),
                }
            }
            _ => Self::banding(op, a, b),
        }
    }

    fn banding(op: BinOp, a: Nilai, b: Nilai) -> Hasil<Nilai> {
        let hasil = match (&a, &b) {
            (Nilai::Bil(x), Nilai::Bil(y)) => Self::banding_ord(op, x.cmp(y)),
            (Nilai::Des(x), Nilai::Des(y)) => match op {
                BinOp::SamaSama => *x == *y,
                BinOp::TidakSama => *x != *y,
                BinOp::KurangDari => *x < *y,
                BinOp::KurangSama => *x <= *y,
                BinOp::LebihDari => *x > *y,
                BinOp::LebihSama => *x >= *y,
                _ => return internal("perbandingan desimal tidak sah"),
            },
            (Nilai::Log(x), Nilai::Log(y)) => match op {
                BinOp::SamaSama => x == y,
                BinOp::TidakSama => x != y,
                _ => return internal("perbandingan logika tidak sah"),
            },
            (Nilai::Teks(x), Nilai::Teks(y)) => {
                let o = x.as_ref().cmp(y.as_ref());
                Self::banding_ord(op, o)
            }
            _ => return internal("operan perbandingan tidak cocok"),
        };
        Ok(Nilai::Log(hasil))
    }

    fn banding_ord(op: BinOp, o: std::cmp::Ordering) -> bool {
        use std::cmp::Ordering::*;
        match op {
            BinOp::SamaSama => o == Equal,
            BinOp::TidakSama => o != Equal,
            BinOp::KurangDari => o == Less,
            BinOp::KurangSama => o != Greater,
            BinOp::LebihDari => o == Greater,
            BinOp::LebihSama => o != Less,
            _ => false,
        }
    }
}

fn tugas_ke_biner(op: TugasOp) -> BinOp {
    match op {
        TugasOp::Tambah => BinOp::Tambah,
        TugasOp::Kurang => BinOp::Kurang,
        TugasOp::Kali => BinOp::Kali,
        TugasOp::Bagi => BinOp::Bagi,
        TugasOp::Mod => BinOp::Mod,
        TugasOp::Isi => BinOp::Tambah,
    }
}

fn bagi_bil(a: i64, b: i64) -> Hasil<i64> {
    if b == 0 {
        return panik("pembagian dengan nol");
    }
    if a == i64::MIN && b == -1 {
        return panik("luapan pembagian");
    }
    Ok((a as i128).div_euclid(b as i128) as i64)
}

fn mod_bil(a: i64, b: i64) -> Hasil<i64> {
    if b == 0 {
        return panik("sisa bagi dengan nol");
    }
    Ok((a as i128).rem_euclid(b as i128) as i64)
}

fn format_nilai(v: &Nilai) -> String {
    match v {
        Nilai::Bil(n) => n.to_string(),
        Nilai::Des(d) => format_des(*d),
        Nilai::Log(b) => {
            if *b {
                "benar".into()
            } else {
                "salah".into()
            }
        }
        Nilai::Teks(s) => s.to_string(),
        Nilai::Daftar(d) => {
            let isi = d.iter().map(format_nilai).collect::<Vec<_>>().join(", ");
            format!("[{}]", isi)
        }
        Nilai::Kosong => String::new(),
    }
}

/// Meniru `printf("%.15g")` lalu menambahkan `.0` untuk bilangan bulat.
fn format_des(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v.is_sign_negative() { "-inf" } else { "inf" }.into();
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    let ilmiah = format!("{:.14e}", v);
    let (mant, exp) = ilmiah.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let neg = mant.starts_with('-');
    let digit = mant.trim_start_matches('-').replace('.', "");
    let digit = digit.trim_end_matches('0');
    let digit = if digit.is_empty() { "0" } else { digit };
    let tanda = if neg { "-" } else { "" };

    if !(-4..15).contains(&exp) {
        let (kepala, ekor) = digit.split_at(1);
        let mut s = String::new();
        s.push_str(tanda);
        s.push_str(kepala);
        if !ekor.is_empty() {
            s.push('.');
            s.push_str(ekor);
        }
        s.push('e');
        s.push(if exp < 0 { '-' } else { '+' });
        s.push_str(&format!("{:02}", exp.abs()));
        s
    } else {
        let titik = exp + 1;
        let mut s = String::new();
        s.push_str(tanda);
        if titik <= 0 {
            s.push_str("0.");
            for _ in 0..(-titik) {
                s.push('0');
            }
            s.push_str(digit);
        } else if titik as usize >= digit.len() {
            s.push_str(digit);
            for _ in digit.len()..(titik as usize) {
                s.push('0');
            }
        } else {
            let (a, b) = digit.split_at(titik as usize);
            s.push_str(a);
            s.push('.');
            s.push_str(b);
        }
        if !s.contains('.') {
            s.push_str(".0");
        }
        s
    }
}
