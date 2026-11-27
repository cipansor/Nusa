//! Penafsir (interpreter) Nusa: menjalankan AST secara langsung tanpa
//! menghasilkan biner. Penjaga runtime, semantik bilangan, dan keluarannya
//! dibuat sama dengan hasil kompilasi LLVM, sehingga `nusa jalan` dan
//! `nusa bangun` memberi perilaku yang setara.

use crate::ast::*;
use crate::sema::{Panggilan, Sema};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

type Sel = Rc<RefCell<Nilai>>;

#[derive(Clone)]
pub enum Nilai {
    Bulat(i64),
    Pecah(f64),
    Log(bool),
    Teks(Rc<str>),
    Larik(Rc<RefCell<Vec<Nilai>>>),
    Irisan {
        data: Rc<RefCell<Vec<Nilai>>>,
        mulai: usize,
        panjang: usize,
    },
    Daftar(Rc<RefCell<Vec<Nilai>>>),
    Struk {
        nama: String,
        medan: Vec<Nilai>,
    },
    Varian {
        nama: String,
        indeks: usize,
    },
    Ref(Sel),
    Kosong,
}

/// Kesalahan runtime: dicetak sebagai `Nusa: ...` lalu keluar dengan kode 1.
pub struct Mati {
    pub pesan: String,
}

type Hasil<T> = Result<T, Mati>;

const KEDALAMAN_MAKS: usize = 2_000;

#[derive(Clone)]
enum Aliran {
    Normal,
    Kembali(Nilai),
    Henti,
    Lanjut,
}

pub struct Interp<'a> {
    prog: &'a Prog,
    sema: &'a Sema,
    jaga: bool,
    takaman: usize,
    lingkup: Vec<HashMap<String, Sel>>,
    kedalaman: usize,
}

pub fn jalan(prog: &Prog, sema: &Sema, jaga: bool) -> i32 {
    let mut it = Interp {
        prog,
        sema,
        jaga,
        takaman: 0,
        lingkup: vec![HashMap::new()],
        kedalaman: 0,
    };
    let hasil = it.mulai();
    let _ = std::io::stdout().flush();
    match hasil {
        Ok(kode) => kode,
        Err(m) => {
            let _ = writeln!(std::io::stderr(), "Nusa: {}", m.pesan);
            1
        }
    }
}

impl<'a> Interp<'a> {
    fn mulai(&mut self) -> Hasil<i32> {
        let utama = match self
            .prog
            .fungsi
            .iter()
            .find(|f| f.nama == "utama" && !f.luar)
        {
            Some(f) => f,
            None => {
                return Err(Mati {
                    pesan: "tidak ada fungsi `utama()` untuk dijalankan".into(),
                });
            }
        };
        if !utama.params.is_empty() {
            return Err(Mati {
                pesan: "`utama` tidak boleh meminta argumen".into(),
            });
        }
        let v = self.panggil(utama, &[])?;
        Ok(match v {
            Nilai::Bulat(n) => (n as u8) as i32,
            _ => 0,
        })
    }

    fn mati(&self, pesan: String) -> Mati {
        Mati { pesan }
    }

    fn pelanggaran(&self, kode: u32, baris: u32) -> Mati {
        let pesan = match kode {
            1 => "luber bilangan (overflow)",
            2 => "indeks di luar batas",
            3 => "pembagian dengan nol",
            4 => "geser tidak sah",
            _ => "pelanggaran runtime",
        };
        Mati {
            pesan: format!("{} pada baris {}", pesan, baris),
        }
    }

    fn jaga(&self, ok: bool, kode: u32, baris: u32) -> Hasil<()> {
        if ok || !self.jaga || self.takaman > 0 {
            Ok(())
        } else {
            Err(self.pelanggaran(kode, baris))
        }
    }

    // ---- lingkungan ------------------------------------------------------

    fn cari_sel(&self, nama: &str) -> Option<Sel> {
        for s in self.lingkup.iter().rev() {
            if let Some(v) = s.get(nama) {
                return Some(v.clone());
            }
        }
        None
    }

    fn sel(&self, nama: &str) -> Hasil<Sel> {
        match self.cari_sel(nama) {
            Some(s) => Ok(s),
            None => Err(self.mati(format!("variabel `{}` tidak ada", nama))),
        }
    }

    fn ikat(&mut self, nama: &str, v: Nilai) {
        if let Some(s) = self.lingkup.last_mut() {
            s.insert(nama.to_string(), Rc::new(RefCell::new(v)));
        }
    }

    fn tulis_sel(&self, s: &Sel, v: Nilai) {
        *s.borrow_mut() = v;
    }

    /// Membaca nilai dari sel, mengikuti satu lapis referensi.
    fn baca_sel(&self, s: &Sel) -> Nilai {
        let v = s.borrow().clone();
        match v {
            Nilai::Ref(dalam) => dalam.borrow().clone(),
            lain => lain,
        }
    }

    fn baca(&self, v: &Nilai) -> Nilai {
        match v {
            Nilai::Ref(s) => s.borrow().clone(),
            lain => lain.clone(),
        }
    }

    // ---- bilangan --------------------------------------------------------

    fn bertanda(t: &Ty) -> bool {
        matches!(t, Ty::Bil | Ty::Bil32 | Ty::Bil8)
    }

    fn rentang(t: &Ty) -> (i128, i128) {
        match t {
            Ty::Bil => (i64::MIN as i128, i64::MAX as i128),
            Ty::Bil32 => (i32::MIN as i128, i32::MAX as i128),
            Ty::Bil8 => (i8::MIN as i128, i8::MAX as i128),
            Ty::Nat => (0, u64::MAX as i128),
            Ty::Nat8 | Ty::Huruf => (0, 255),
            Ty::Log => (0, 1),
            _ => (i64::MIN as i128, i64::MAX as i128),
        }
    }

    fn ke_i128(v: i64, t: &Ty) -> i128 {
        if Self::bertanda(t) {
            v as i128
        } else {
            (v as u64) as i128
        }
    }

    fn lipat(x: i128, t: &Ty) -> i128 {
        match t {
            Ty::Bil8 => (x as i8) as i128,
            Ty::Nat8 | Ty::Huruf => (x as u8) as i128,
            Ty::Bil32 => (x as i32) as i128,
            Ty::Nat => (x as u64) as i128,
            Ty::Log => (x != 0) as i128,
            _ => x,
        }
    }

    fn dari_i128(x: i128, t: &Ty) -> i64 {
        if Self::bertanda(t) {
            x as i64
        } else {
            match t {
                Ty::Nat8 | Ty::Huruf => x as u8 as i64,
                Ty::Nat => x as u64 as i64,
                _ => x as u64 as i64,
            }
        }
    }

    fn arit(&self, op: BinOp, a: i64, b: i64, t: &Ty, baris: u32) -> Hasil<i64> {
        let (x, y) = (Self::ke_i128(a, t), Self::ke_i128(b, t));
        let (lo, hi) = Self::rentang(t);
        let r = match op {
            BinOp::Tambah => x + y,
            BinOp::Kurang => x - y,
            BinOp::Kali => x * y,
            _ => x,
        };
        self.jaga(r >= lo && r <= hi, 1, baris)?;
        Ok(Self::dari_i128(Self::lipat(r, t), t))
    }

    fn bagi(&self, op: BinOp, a: i64, b: i64, t: &Ty, baris: u32) -> Hasil<i64> {
        let (x, y) = (Self::ke_i128(a, t), Self::ke_i128(b, t));
        self.jaga(y != 0, 3, baris)?;
        if Self::bertanda(t) {
            let (lo, _) = Self::rentang(t);
            if x == lo && y == -1 {
                return Err(self.pelanggaran(1, baris));
            }
        }
        let r = if Self::bertanda(t) {
            match x.checked_div(y) {
                Some(q) if op == BinOp::Bagi => q,
                None => return Err(self.pelanggaran(1, baris)),
                _ => match x.checked_rem(y) {
                    Some(m) => m,
                    None => return Err(self.pelanggaran(1, baris)),
                },
            }
        } else if op == BinOp::Bagi {
            x / y
        } else {
            x % y
        };
        Ok(Self::dari_i128(Self::lipat(r, t), t))
    }

    // ---- pernyataan ------------------------------------------------------

    fn blok(&mut self, b: &Block) -> Hasil<Aliran> {
        self.lingkup.push(HashMap::new());
        let mut hasil = Ok(Aliran::Normal);
        for st in &b.stmts {
            match self.stmt(st) {
                Ok(Aliran::Normal) => {}
                Ok(lain) => {
                    hasil = Ok(lain);
                    break;
                }
                Err(e) => {
                    hasil = Err(e);
                    break;
                }
            }
        }
        self.lingkup.pop();
        hasil
    }

    fn stmt(&mut self, st: &Stmt) -> Hasil<Aliran> {
        match st {
            Stmt::Ikat { nama, nilai, .. } => {
                let v = self.eks(nilai)?;
                self.ikat(nama, v);
                Ok(Aliran::Normal)
            }
            Stmt::Tugas {
                target,
                nilai,
                span,
            } => {
                let v = self.eks(nilai)?;
                match &target.kind {
                    ExprKind::Var(nama) => {
                        let s = self.sel(nama)?;
                        self.tulis_sel(&s, v);
                    }
                    ExprKind::Indeks { dasar, indeks } => {
                        let wadah = self.eks(dasar)?;
                        let i = self.eks_bulat(indeks)?;
                        self.tulis_unsur(&wadah, i, v, span.baris)?;
                    }
                    ExprKind::Medan { dasar, nama } => {
                        let mut w = self.eks(dasar)?;
                        if let Nilai::Struk { nama: sn, medan } = &mut w {
                            if let Some(idx) = self
                                .sema
                                .struk
                                .get(sn)
                                .and_then(|s| s.indeks.get(nama).copied())
                            {
                                if let Some(slot) = medan.get_mut(idx) {
                                    *slot = v;
                                    self.tulis_tempat(dasar, w)?;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                Ok(Aliran::Normal)
            }
            Stmt::Jika {
                kond, maka, lain, ..
            } => {
                if self.eks_log(kond)? {
                    self.blok(maka)
                } else if let Some(l) = lain {
                    self.blok(l)
                } else {
                    Ok(Aliran::Normal)
                }
            }
            Stmt::Selama { kond, badan, .. } => {
                loop {
                    if !self.eks_log(kond)? {
                        break;
                    }
                    match self.blok(badan)? {
                        Aliran::Henti => break,
                        Aliran::Kembali(v) => return Ok(Aliran::Kembali(v)),
                        _ => {}
                    }
                }
                Ok(Aliran::Normal)
            }
            Stmt::UntukRentang {
                nama,
                dari,
                sampai,
                inklusif,
                badan,
                ..
            } => {
                let awal = self.eks_bulat(dari)?;
                let akhir = self.eks_bulat(sampai)?;
                let mut i = awal;
                loop {
                    let lanjut = if *inklusif { i <= akhir } else { i < akhir };
                    if !lanjut {
                        break;
                    }
                    let s = Rc::new(RefCell::new(Nilai::Bulat(i)));
                    self.lingkup.push(HashMap::from([(nama.clone(), s)]));
                    let aliran = self.blok(badan);
                    self.lingkup.pop();
                    match aliran? {
                        Aliran::Henti => break,
                        Aliran::Kembali(v) => return Ok(Aliran::Kembali(v)),
                        _ => {}
                    }
                    match i.checked_add(1) {
                        Some(n) => i = n,
                        None => break,
                    }
                }
                Ok(Aliran::Normal)
            }
            Stmt::UntukIsi {
                nama, wadah, badan, ..
            } => {
                let w = self.eks(wadah)?;
                let (data, mulai, panjang) = match &w {
                    Nilai::Larik(d) => (d.clone(), 0, d.borrow().len()),
                    Nilai::Irisan {
                        data,
                        mulai,
                        panjang,
                    } => (data.clone(), *mulai, *panjang),
                    Nilai::Daftar(d) => (d.clone(), 0, d.borrow().len()),
                    Nilai::Teks(t) => {
                        let bytes: Vec<Nilai> = t.bytes().map(|b| Nilai::Bulat(b as i64)).collect();
                        (Rc::new(RefCell::new(bytes)), 0, t.len())
                    }
                    _ => (Rc::new(RefCell::new(Vec::new())), 0, 0),
                };
                let mut i = 0usize;
                while i < panjang {
                    let el = data
                        .borrow()
                        .get(mulai + i)
                        .cloned()
                        .unwrap_or(Nilai::Kosong);
                    let s = Rc::new(RefCell::new(el));
                    self.lingkup.push(HashMap::from([(nama.clone(), s)]));
                    let aliran = self.blok(badan);
                    self.lingkup.pop();
                    match aliran? {
                        Aliran::Henti => break,
                        Aliran::Kembali(v) => return Ok(Aliran::Kembali(v)),
                        _ => {}
                    }
                    i += 1;
                }
                Ok(Aliran::Normal)
            }
            Stmt::Kembali(nilai, _) => {
                let v = match nilai {
                    Some(e) => self.eks(e)?,
                    None => Nilai::Kosong,
                };
                Ok(Aliran::Kembali(v))
            }
            Stmt::Tegas { kond, pesan, span } => {
                if !self.eks_log(kond)? {
                    let tambahan = match pesan {
                        Some(p) => format!(": {}", p),
                        None => String::new(),
                    };
                    return Err(self.mati(format!(
                        "`tegas` gagal pada baris {}{}",
                        span.baris, tambahan
                    )));
                }
                Ok(Aliran::Normal)
            }
            Stmt::Ekspresi(e) => {
                self.eks(e)?;
                Ok(Aliran::Normal)
            }
            Stmt::Henti(_) => Ok(Aliran::Henti),
            Stmt::Lanjut(_) => Ok(Aliran::Lanjut),
            Stmt::Cocok {
                nilai,
                kasus,
                lainnya,
                ..
            } => {
                let v = self.eks(nilai)?;
                let (enum_nama, indeks) = match v {
                    Nilai::Varian { nama, indeks } => (nama, indeks),
                    _ => return Err(self.mati("`cocok` memerlukan nilai enumerasi".into())),
                };
                let varian = self
                    .sema
                    .enumerasi
                    .get(&enum_nama)
                    .and_then(|e| e.varian.get(indeks).cloned());
                for (nama, b) in kasus {
                    if Some(nama.as_str()) == varian.as_deref() {
                        return self.blok(b);
                    }
                }
                match lainnya {
                    Some(b) => self.blok(b),
                    None => Err(self.mati("tidak ada `kasus` yang cocok".into())),
                }
            }
            Stmt::Blok(b) => self.blok(b),
            Stmt::Takaman(b) => {
                self.takaman += 1;
                let r = self.blok(b);
                self.takaman -= 1;
                r
            }
        }
    }

    fn tulis_tempat(&mut self, tempat: &Expr, v: Nilai) -> Hasil<()> {
        match &tempat.kind {
            ExprKind::Var(nama) => {
                let s = self.sel(nama)?;
                self.tulis_sel(&s, v);
                Ok(())
            }
            ExprKind::Medan { dasar, nama } => {
                let mut w = self.eks(dasar)?;
                if let Nilai::Struk { nama: sn, medan } = &mut w {
                    if let Some(idx) = self
                        .sema
                        .struk
                        .get(sn)
                        .and_then(|s| s.indeks.get(nama).copied())
                    {
                        if let Some(slot) = medan.get_mut(idx) {
                            *slot = v;
                        }
                    }
                }
                self.tulis_tempat(dasar, w)
            }
            _ => Ok(()),
        }
    }

    // ---- wadah -----------------------------------------------------------

    fn batas(&self, i: i64, n: usize, baris: u32) -> Hasil<usize> {
        let ok = i >= 0 && (i as u128) < (n as u128);
        self.jaga(ok, 2, baris)?;
        if ok {
            Ok(i as usize)
        } else {
            Ok(0)
        }
    }

    fn unsur(&self, wadah: &Nilai, i: i64, baris: u32) -> Hasil<Nilai> {
        match wadah {
            Nilai::Larik(d) => {
                let idx = self.batas(i, d.borrow().len(), baris)?;
                Ok(d.borrow().get(idx).cloned().unwrap_or(Nilai::Kosong))
            }
            Nilai::Irisan {
                data,
                mulai,
                panjang,
            } => {
                let idx = self.batas(i, *panjang, baris)?;
                Ok(data
                    .borrow()
                    .get(mulai + idx)
                    .cloned()
                    .unwrap_or(Nilai::Kosong))
            }
            Nilai::Daftar(d) => {
                let idx = self.batas(i, d.borrow().len(), baris)?;
                Ok(d.borrow().get(idx).cloned().unwrap_or(Nilai::Kosong))
            }
            Nilai::Teks(t) => {
                let idx = self.batas(i, t.len(), baris)?;
                Ok(Nilai::Bulat(t.as_bytes()[idx] as i64))
            }
            Nilai::Ref(s) => {
                let dalam = s.borrow().clone();
                self.unsur(&dalam, i, baris)
            }
            _ => Err(self.mati("nilai ini tidak dapat diindeks".into())),
        }
    }

    fn tulis_unsur(&self, wadah: &Nilai, i: i64, v: Nilai, baris: u32) -> Hasil<()> {
        match wadah {
            Nilai::Larik(d) => {
                let idx = self.batas(i, d.borrow().len(), baris)?;
                if let Some(slot) = d.borrow_mut().get_mut(idx) {
                    *slot = v;
                }
                Ok(())
            }
            Nilai::Irisan {
                data,
                mulai,
                panjang,
            } => {
                let idx = self.batas(i, *panjang, baris)?;
                if let Some(slot) = data.borrow_mut().get_mut(mulai + idx) {
                    *slot = v;
                }
                Ok(())
            }
            Nilai::Daftar(d) => {
                let idx = self.batas(i, d.borrow().len(), baris)?;
                if let Some(slot) = d.borrow_mut().get_mut(idx) {
                    *slot = v;
                }
                Ok(())
            }
            Nilai::Ref(s) => {
                let dalam = s.borrow().clone();
                self.tulis_unsur(&dalam, i, v, baris)
            }
            _ => Err(self.mati("nilai ini tidak dapat diindeks".into())),
        }
    }

    fn panjang(&self, wadah: &Nilai) -> i64 {
        match wadah {
            Nilai::Larik(d) => d.borrow().len() as i64,
            Nilai::Irisan { panjang, .. } => *panjang as i64,
            Nilai::Daftar(d) => d.borrow().len() as i64,
            Nilai::Teks(t) => t.len() as i64,
            Nilai::Ref(s) => {
                let dalam = s.borrow().clone();
                self.panjang(&dalam)
            }
            _ => 0,
        }
    }

    // ---- ekspresi --------------------------------------------------------

    fn tipe(&self, e: &Expr) -> Ty {
        self.sema.tipe.get(&e.id).cloned().unwrap_or(Ty::Bil)
    }

    fn eks_bulat(&mut self, e: &Expr) -> Hasil<i64> {
        match self.eks(e)? {
            Nilai::Bulat(n) => Ok(n),
            Nilai::Log(b) => Ok(b as i64),
            Nilai::Ref(s) => match s.borrow().clone() {
                Nilai::Bulat(n) => Ok(n),
                _ => Err(self.mati("diharapkan bilangan".into())),
            },
            _ => Err(self.mati("diharapkan bilangan".into())),
        }
    }

    fn eks_log(&mut self, e: &Expr) -> Hasil<bool> {
        match self.eks(e)? {
            Nilai::Log(b) => Ok(b),
            Nilai::Bulat(n) => Ok(n != 0),
            _ => Err(self.mati("diharapkan nilai logika".into())),
        }
    }

    fn eks(&mut self, e: &Expr) -> Hasil<Nilai> {
        let ty = self.tipe(e);
        match &e.kind {
            ExprKind::Bil(v) => Ok(Nilai::Bulat(*v)),
            ExprKind::Huruf(v) => Ok(Nilai::Bulat(*v as i64)),
            ExprKind::Pecah(v) => Ok(Nilai::Pecah(*v)),
            ExprKind::Teks(s) => Ok(Nilai::Teks(Rc::from(s.as_str()))),
            ExprKind::Log(b) => Ok(Nilai::Log(*b)),
            ExprKind::Var(n) => {
                let s = self.sel(n)?;
                Ok(self.baca_sel(&s))
            }
            ExprKind::Biner(op, a, b) => self.eks_biner(*op, a, b, &ty, e.span.baris),
            ExprKind::Uner(op, a) => match op {
                UnOp::Bukan => Ok(Nilai::Log(!self.eks_log(a)?)),
                UnOp::Neg => {
                    let v = self.eks(a)?;
                    match v {
                        Nilai::Pecah(f) => Ok(Nilai::Pecah(-f)),
                        Nilai::Bulat(n) => {
                            let x = Self::ke_i128(n, &ty);
                            let (lo, hi) = Self::rentang(&ty);
                            self.jaga(-x >= lo && -x <= hi, 1, e.span.baris)?;
                            Ok(Nilai::Bulat(Self::dari_i128(Self::lipat(-x, &ty), &ty)))
                        }
                        _ => Err(self.mati("`-` memerlukan bilangan".into())),
                    }
                }
            },
            ExprKind::Indeks { dasar, indeks } => {
                let w = self.eks(dasar)?;
                let i = self.eks_bulat(indeks)?;
                self.unsur(&w, i, e.span.baris)
            }
            ExprKind::Medan { dasar, nama } => {
                let w = self.eks(dasar)?;
                match &w {
                    Nilai::Struk { nama: sn, medan } => match self
                        .sema
                        .struk
                        .get(sn)
                        .and_then(|s| s.indeks.get(nama).copied())
                    {
                        Some(i) => Ok(medan.get(i).cloned().unwrap_or(Nilai::Kosong)),
                        None => {
                            Err(self.mati(format!("struk `{}` tidak punya medan `{}`", sn, nama)))
                        }
                    },
                    _ => Err(self.mati(format!("nilai bukan struk, tidak punya medan `{}`", nama))),
                }
            }
            ExprKind::Alamat { dasar, .. } => {
                let s = self.sel_argumen(dasar)?;
                Ok(Nilai::Ref(s))
            }
            ExprKind::Sebagai { nilai, ty: ke } => {
                let v = self.eks(nilai)?;
                let dari = self.tipe(nilai);
                self.konversi(v, &dari, ke, e.span.baris)
            }
            ExprKind::StrukLit { nama, medan } => {
                let info = match self.sema.struk.get(nama) {
                    Some(s) => s.clone(),
                    None => return Err(self.mati(format!("struk `{}` tidak dikenal", nama))),
                };
                let mut isi = Vec::with_capacity(info.medan.len());
                for (mn, _fty) in &info.medan {
                    let mut nilai = Nilai::Kosong;
                    for (nm, me) in medan {
                        if nm == mn {
                            nilai = self.eks(me)?;
                            break;
                        }
                    }
                    isi.push(nilai);
                }
                Ok(Nilai::Struk {
                    nama: nama.clone(),
                    medan: isi,
                })
            }
            ExprKind::Varian { enumerasi, varian } => {
                let idx = self
                    .sema
                    .enumerasi
                    .get(enumerasi)
                    .and_then(|e| e.varian.iter().position(|v| v == varian))
                    .unwrap_or(0);
                Ok(Nilai::Varian {
                    nama: enumerasi.clone(),
                    indeks: idx,
                })
            }
            ExprKind::ArrayLit(isi) => {
                let mut v = Vec::with_capacity(isi.len());
                for x in isi {
                    v.push(self.eks(x)?);
                }
                Ok(Nilai::Larik(Rc::new(RefCell::new(v))))
            }
            ExprKind::Panggil { nama: _, args } => self.panggil_eks(e, args),
        }
    }

    fn eks_biner(&mut self, op: BinOp, a: &Expr, b: &Expr, ty: &Ty, baris: u32) -> Hasil<Nilai> {
        match op {
            BinOp::Dan => {
                if !self.eks_log(a)? {
                    return Ok(Nilai::Log(false));
                }
                return Ok(Nilai::Log(self.eks_log(b)?));
            }
            BinOp::Atau => {
                if self.eks_log(a)? {
                    return Ok(Nilai::Log(true));
                }
                return Ok(Nilai::Log(self.eks_log(b)?));
            }
            _ => {}
        }
        if *ty == Ty::Log {
            return self.banding(op, a, b);
        }
        if matches!(op, BinOp::GeserKiri | BinOp::GeserKanan) {
            let x = self.eks(a)?;
            let y = self.eks(b)?;
            let (p, q) = (self.ke_bulat(&x)?, self.ke_bulat(&y)?);
            let lebar: i64 = match ty {
                Ty::Bil8 | Ty::Nat8 => 8,
                Ty::Bil32 => 32,
                _ => 64,
            };
            self.jaga(q >= 0 && q < lebar, 4, baris)?;
            let r: i128 = if op == BinOp::GeserKiri {
                if Self::bertanda(ty) {
                    (p as i128) << q
                } else {
                    ((p as u64) << q) as i128
                }
            } else if Self::bertanda(ty) {
                (p >> q) as i128
            } else {
                ((p as u64) >> q) as i128
            };
            return Ok(Nilai::Bulat(Self::dari_i128(Self::lipat(r, ty), ty)));
        }
        let x = self.eks(a)?;
        let y = self.eks(b)?;
        if *ty == Ty::Pecah {
            let (p, q) = (self.ke_pecah(&x), self.ke_pecah(&y));
            let r = match op {
                BinOp::Tambah => p + q,
                BinOp::Kurang => p - q,
                BinOp::Kali => p * q,
                BinOp::Bagi => p / q,
                BinOp::Sisa => p % q,
                _ => p,
            };
            return Ok(Nilai::Pecah(r));
        }
        let (p, q) = (self.ke_bulat(&x)?, self.ke_bulat(&y)?);
        match op {
            BinOp::Bagi | BinOp::Sisa => Ok(Nilai::Bulat(self.bagi(op, p, q, ty, baris)?)),
            _ => Ok(Nilai::Bulat(self.arit(op, p, q, ty, baris)?)),
        }
    }

    fn ke_bulat(&self, v: &Nilai) -> Hasil<i64> {
        match self.baca(v) {
            Nilai::Bulat(n) => Ok(n),
            _ => Err(self.mati("diharapkan bilangan bulat".into())),
        }
    }

    fn ke_pecah(&self, v: &Nilai) -> f64 {
        match self.baca(v) {
            Nilai::Pecah(f) => f,
            Nilai::Bulat(n) => n as f64,
            _ => 0.0,
        }
    }

    fn banding(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Hasil<Nilai> {
        let t = self.tipe(a);
        let x = self.eks(a)?;
        let y = self.eks(b)?;
        let hasil = match t {
            Ty::Teks => {
                let (p, q) = (self.ke_teks(&x), self.ke_teks(&y));
                match op {
                    BinOp::Sama => p == q,
                    BinOp::TidakSama => p != q,
                    BinOp::Kecil => p < q,
                    BinOp::KecilSama => p <= q,
                    BinOp::Besar => p > q,
                    BinOp::BesarSama => p >= q,
                    _ => false,
                }
            }
            Ty::Pecah => {
                let (p, q) = (self.ke_pecah(&x), self.ke_pecah(&y));
                match op {
                    BinOp::Sama => p == q,
                    BinOp::TidakSama => p != q,
                    BinOp::Kecil => p < q,
                    BinOp::KecilSama => p <= q,
                    BinOp::Besar => p > q,
                    BinOp::BesarSama => p >= q,
                    _ => false,
                }
            }
            Ty::Log => {
                let (p, q) = (matches!(x, Nilai::Log(true)), matches!(y, Nilai::Log(true)));
                match op {
                    BinOp::Sama => p == q,
                    BinOp::TidakSama => p != q,
                    _ => false,
                }
            }
            Ty::Nama(_) => match op {
                BinOp::Sama => self.sama(&x, &y),
                BinOp::TidakSama => !self.sama(&x, &y),
                _ => false,
            },
            _ => {
                let p = Self::ke_i128(self.ke_bulat(&x)?, &t);
                let q = Self::ke_i128(self.ke_bulat(&y)?, &t);
                match op {
                    BinOp::Sama => p == q,
                    BinOp::TidakSama => p != q,
                    BinOp::Kecil => p < q,
                    BinOp::KecilSama => p <= q,
                    BinOp::Besar => p > q,
                    BinOp::BesarSama => p >= q,
                    _ => false,
                }
            }
        };
        Ok(Nilai::Log(hasil))
    }

    fn ke_teks(&self, v: &Nilai) -> String {
        match self.baca(v) {
            Nilai::Teks(t) => t.to_string(),
            lainnya => self.render(&lainnya, &Ty::Teks),
        }
    }

    fn sama(&self, a: &Nilai, b: &Nilai) -> bool {
        match (self.baca(a), self.baca(b)) {
            (Nilai::Bulat(x), Nilai::Bulat(y)) => x == y,
            (Nilai::Pecah(x), Nilai::Pecah(y)) => x == y,
            (Nilai::Log(x), Nilai::Log(y)) => x == y,
            (Nilai::Teks(x), Nilai::Teks(y)) => x == y,
            (
                Nilai::Varian {
                    nama: n1,
                    indeks: i1,
                },
                Nilai::Varian {
                    nama: n2,
                    indeks: i2,
                },
            ) => n1 == n2 && i1 == i2,
            (
                Nilai::Struk {
                    nama: n1,
                    medan: m1,
                },
                Nilai::Struk {
                    nama: n2,
                    medan: m2,
                },
            ) => {
                n1 == n2
                    && m1.len() == m2.len()
                    && m1.iter().zip(m2.iter()).all(|(x, y)| self.sama(x, y))
            }
            (Nilai::Larik(x), Nilai::Larik(y)) => {
                let (x, y) = (x.borrow(), y.borrow());
                x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| self.sama(p, q))
            }
            (Nilai::Daftar(x), Nilai::Daftar(y)) => {
                let (x, y) = (x.borrow(), y.borrow());
                x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| self.sama(p, q))
            }
            (
                Nilai::Irisan {
                    data: d1,
                    mulai: m1,
                    panjang: p1,
                },
                Nilai::Irisan {
                    data: d2,
                    mulai: m2,
                    panjang: p2,
                },
            ) => {
                if p1 != p2 {
                    return false;
                }
                let (a, b) = (d1.borrow(), d2.borrow());
                (0..p1).all(|i| match (a.get(m1 + i), b.get(m2 + i)) {
                    (Some(x), Some(y)) => self.sama(x, y),
                    _ => false,
                })
            }
            (Nilai::Kosong, Nilai::Kosong) => true,
            _ => false,
        }
    }

    fn konversi(&self, v: Nilai, dari: &Ty, ke: &Ty, baris: u32) -> Hasil<Nilai> {
        if dari == ke {
            return Ok(v);
        }
        if *ke == Ty::Pecah {
            let x = match self.baca(&v) {
                Nilai::Pecah(f) => f,
                Nilai::Bulat(n) => Self::ke_i128(n, dari) as f64,
                _ => 0.0,
            };
            return Ok(Nilai::Pecah(x));
        }
        if *dari == Ty::Pecah {
            let f = self.ke_pecah(&v);
            if f.is_nan() {
                self.jaga(false, 1, baris)?;
            }
            let x = f.trunc() as i128;
            let (lo, hi) = Self::rentang(ke);
            self.jaga(x >= lo && x <= hi, 1, baris)?;
            return Ok(Nilai::Bulat(Self::dari_i128(Self::lipat(x, ke), ke)));
        }
        let n = self.ke_bulat(&v)?;
        let x = Self::ke_i128(n, dari);
        let (lo, hi) = Self::rentang(ke);
        self.jaga(x >= lo && x <= hi, 1, baris)?;
        Ok(Nilai::Bulat(Self::dari_i128(Self::lipat(x, ke), ke)))
    }

    // ---- pemanggilan -----------------------------------------------------

    fn panggil_eks(&mut self, e: &Expr, args: &[Expr]) -> Hasil<Nilai> {
        let jenis = self.sema.panggil.get(&e.id).cloned();
        match jenis {
            Some(Panggilan::Bawaan(n)) => self.bawaan(&n, args, e.span.baris),
            Some(Panggilan::Fungsi(n)) => {
                let prog = self.prog;
                match prog.fungsi.iter().find(|f| f.nama == n) {
                    Some(f) => self.panggil(f, args),
                    None => Err(self.mati(format!("fungsi `{}` tidak ditemukan", n))),
                }
            }
            None => Err(self.mati("pemanggilan tidak dikenali".into())),
        }
    }

    fn panggil(&mut self, f: &'a Fungsi, args: &[Expr]) -> Hasil<Nilai> {
        self.kedalaman += 1;
        if self.kedalaman >= KEDALAMAN_MAKS {
            self.kedalaman -= 1;
            return Err(self.mati("rekursi terlalu dalam".into()));
        }
        let mut lingkup: HashMap<String, Sel> = HashMap::new();
        for (i, p) in f.params.iter().enumerate() {
            let arg = match args.get(i) {
                Some(a) => a,
                None => {
                    self.kedalaman -= 1;
                    return Err(self.mati(format!(
                        "`{}` memerlukan {} argumen",
                        f.nama,
                        f.params.len()
                    )));
                }
            };
            let sel = if matches!(p.ty, Ty::Ref { .. }) {
                self.sel_argumen(arg)?
            } else {
                let v = self.eks(arg)?;
                Rc::new(RefCell::new(self.sesuaikan(v, &p.ty)))
            };
            lingkup.insert(p.nama.clone(), sel);
        }
        self.lingkup.push(lingkup);
        let hasil = self.blok(&f.badan);
        self.lingkup.pop();
        self.kedalaman -= 1;
        match hasil? {
            Aliran::Kembali(v) => Ok(v),
            _ => Ok(Nilai::Kosong),
        }
    }

    /// Menyesuaikan nilai dengan tipe parameter (larik -> irisan).
    fn sesuaikan(&self, v: Nilai, harap: &Ty) -> Nilai {
        match (harap, v) {
            (Ty::Irisan(_), Nilai::Larik(d)) => {
                let n = d.borrow().len();
                Nilai::Irisan {
                    data: d,
                    mulai: 0,
                    panjang: n,
                }
            }
            (_, v) => v,
        }
    }

    /// Sel (tempat) dari sebuah argumen: untuk `&x`/`&ubah x` memakai sel
    /// variabel aslinya sehingga penulisan di dalam fungsi terlihat pemanggil.
    fn sel_argumen(&mut self, e: &Expr) -> Hasil<Sel> {
        match &e.kind {
            ExprKind::Alamat { dasar, .. } => self.sel_ekspresi(dasar),
            ExprKind::Var(n) => self.sel(n),
            _ => {
                let v = self.eks(e)?;
                match v {
                    Nilai::Ref(s) => Ok(s),
                    lain => Ok(Rc::new(RefCell::new(lain))),
                }
            }
        }
    }

    fn sel_ekspresi(&mut self, e: &Expr) -> Hasil<Sel> {
        match &e.kind {
            ExprKind::Var(n) => self.sel(n),
            _ => {
                let v = self.eks(e)?;
                Ok(Rc::new(RefCell::new(v)))
            }
        }
    }

    // ---- bawaan ----------------------------------------------------------

    fn bawaan(&mut self, nama: &str, args: &[Expr], baris: u32) -> Hasil<Nilai> {
        match nama {
            "cetak" | "tulis" => {
                let mut teks = String::new();
                for a in args {
                    let v = self.eks(a)?;
                    let t = self.tipe(a);
                    teks.push_str(&self.render(&v, &t));
                }
                if nama == "cetak" {
                    teks.push('\n');
                }
                print!("{}", teks);
                Ok(Nilai::Kosong)
            }
            "panjang" => {
                let w = self.eks(&args[0])?;
                Ok(Nilai::Bulat(self.panjang(&w)))
            }
            "ambil" => {
                let w = self.eks(&args[0])?;
                let i = self.eks_bulat(&args[1])?;
                self.unsur(&w, i, baris)
            }
            "daftar" => Ok(Nilai::Daftar(Rc::new(RefCell::new(Vec::new())))),
            "tambah" => {
                let sel = self.sel_argumen(&args[0])?;
                let v = self.eks(&args[1])?;
                let d = match &*sel.borrow() {
                    Nilai::Daftar(d) => d.clone(),
                    _ => return Err(self.mati("`tambah` memerlukan `Daftar<T>`".into())),
                };
                d.borrow_mut().push(v);
                Ok(Nilai::Kosong)
            }
            "sisip" => {
                let sel = self.sel_argumen(&args[0])?;
                let i = self.eks_bulat(&args[1])? as usize;
                let v = self.eks(&args[2])?;
                let d = match &*sel.borrow() {
                    Nilai::Daftar(d) => d.clone(),
                    _ => return Err(self.mati("`sisip` memerlukan `Daftar<T>`".into())),
                };
                let panjang = d.borrow().len();
                if i > panjang {
                    return Err(self.pelanggaran(2, baris));
                }
                d.borrow_mut().insert(i, v);
                Ok(Nilai::Kosong)
            }
            "gagal" => {
                let v = self.eks(&args[0])?;
                let pesan = self.ke_teks(&v);
                Err(Mati { pesan })
            }
            lain => Err(self.mati(format!("bawaan `{}` belum dikenal penafsir", lain))),
        }
    }

    // ---- penyajian -------------------------------------------------------

    fn render(&self, v: &Nilai, ty: &Ty) -> String {
        let v = self.baca(v);
        match (&v, ty) {
            (Nilai::Teks(t), _) => t.to_string(),
            (Nilai::Log(b), _) => {
                if *b {
                    "benar".into()
                } else {
                    "salah".into()
                }
            }
            (Nilai::Pecah(f), _) => fmt_pecah(*f),
            (Nilai::Bulat(n), Ty::Nat) | (Nilai::Bulat(n), Ty::Nat8) => format!("{}", *n as u64),
            (Nilai::Bulat(n), Ty::Huruf) => {
                let b = (*n as u64) as u8;
                (b as char).to_string()
            }
            (Nilai::Varian { nama, indeks }, _) => self
                .sema
                .enumerasi
                .get(nama)
                .and_then(|e| e.varian.get(*indeks).cloned())
                .unwrap_or_else(|| format!("{}::{}", nama, indeks)),
            (Nilai::Bulat(n), _) => format!("{}", n),
            (Nilai::Kosong, _) => String::new(),
            _ => String::new(),
        }
    }
}

fn fmt_pecah(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let pangkat = v.abs().log10().floor() as i32;
    if !(-4..6).contains(&pangkat) {
        let mantissa = v / 10f64.powi(pangkat);
        let m = format!("{:.5}", mantissa);
        let m = m.trim_end_matches('0').trim_end_matches('.').to_string();
        format!(
            "{}e{}{:02}",
            m,
            if pangkat < 0 { "-" } else { "+" },
            pangkat.abs()
        )
    } else {
        let desimal = (5 - pangkat).max(0) as usize;
        let s = format!("{:.*}", desimal, v);
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    }
}
