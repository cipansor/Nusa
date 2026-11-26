//! Verifikasi formal: membangkitkan kondisi verifikasi (VC) dari kontrak
//! `perlu`/`jamin`/`invarian`/`tegas` dan pemeriksaan keamanan (batas indeks,
//! luber bilangan, bagi nol), lalu memeriksanya dengan Z3 melalui SMT-LIB2.

use crate::ast::*;
use crate::sema::{Panggilan, Sema};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hasil {
    Terbukti,
    Gagal,
    TidakDiketahui,
}

pub struct Tugas {
    pub deskripsi: String,
    pub baris: u32,
    pub hasil: Hasil,
    pub model: Option<String>,
}

pub struct HasilFungsi {
    pub nama: String,
    pub tugas: Vec<Tugas>,
    pub dilewati: Option<String>,
    pub catatan: Vec<String>,
}

pub struct Laporan {
    pub fungsi: Vec<HasilFungsi>,
    pub terbukti: usize,
    pub gagal: usize,
    pub tidak_diketahui: usize,
    pub dilewati: usize,
}

impl Laporan {
    pub fn cetak(&self) {
        println!("verifikasi formal Nusa (SMT-LIB2 + Z3)");
        for f in &self.fungsi {
            if let Some(alasan) = &f.dilewati {
                println!("  [lewat] {}: {}", f.nama, alasan);
                continue;
            }
            if f.tugas.is_empty() && f.catatan.is_empty() {
                println!("  [ok]    {}: tidak ada tugas", f.nama);
                continue;
            }
            println!("  [{}] {}", if f.tugas.iter().all(|t| t.hasil == Hasil::Terbukti) { "ok" } else { "!!" }, f.nama);
            for c in &f.catatan {
                println!("          catatan: {}", c);
            }
            for t in &f.tugas {
                let label = match t.hasil {
                    Hasil::Terbukti => "terbukti ",
                    Hasil::Gagal => "TAK TERBUKTI",
                    Hasil::TidakDiketahui => "tidak tahu",
                };
                println!("          {} baris {:<4} {}", label, t.baris, t.deskripsi);
                if let Some(m) = &t.model {
                    if t.hasil != Hasil::Terbukti {
                        println!("                    contoh penyangkal: {}", m);
                    }
                }
            }
        }
        println!(
            "ringkasan: {} tugas, {} terbukti, {} tak terbukti, {} tidak diketahui, {} fungsi dilewati",
            self.terbukti + self.gagal + self.tidak_diketahui,
            self.terbukti,
            self.gagal,
            self.tidak_diketahui,
            self.dilewati
        );
    }
}

#[derive(Clone)]
struct Nl {
    term: String,
    ty: Ty,
    panjang: Option<String>,
}

type Env = HashMap<String, Nl>;

struct Vc<'a> {
    sema: &'a Sema,
    prog: &'a Prog,
    fungsi: &'a Fungsi,
    decl: Vec<String>,
    tugas: Vec<Tugas>,
    catatan: Vec<String>,
    versi: HashMap<String, u32>,
    spek: bool,
}

pub fn periksa(prog: &Prog, sema: &Sema, _berkas: &str) -> Laporan {
    let mut lap = Laporan { fungsi: Vec::new(), terbukti: 0, gagal: 0, tidak_diketahui: 0, dilewati: 0 };
    for f in &prog.fungsi {
        if f.luar {
            continue;
        }
        let mut alasan = None;
        for p in &f.params {
            if !boleh_verifikasi(&p.ty) {
                alasan = Some(format!("parameter `{}` bertipe `{}` belum dimodelkan", p.nama, p.ty.sebutan()));
            }
        }
        if !boleh_verifikasi(&f.hasil) {
            alasan = Some(format!("tipe hasil `{}` belum dimodelkan", f.hasil.sebutan()));
        }
        if let Some(a) = &alasan {
            lap.fungsi.push(HasilFungsi { nama: f.nama.clone(), tugas: vec![], dilewati: Some(a.clone()), catatan: vec![] });
            lap.dilewati += 1;
            continue;
        }
        let mut vc = Vc {
            sema,
            prog,
            fungsi: f,
            decl: Vec::new(),
            tugas: Vec::new(),
            catatan: Vec::new(),
            versi: HashMap::new(),
            spek: false,
        };
        let hasil = vc.jalankan();
        lap.terbukti += hasil.tugas.iter().filter(|t| t.hasil == Hasil::Terbukti).count();
        lap.gagal += hasil.tugas.iter().filter(|t| t.hasil == Hasil::Gagal).count();
        lap.tidak_diketahui += hasil.tugas.iter().filter(|t| t.hasil == Hasil::TidakDiketahui).count();
        lap.fungsi.push(hasil);
    }
    lap
}

fn boleh_verifikasi(t: &Ty) -> bool {
    match t {
        Ty::Bil | Ty::Bil32 | Ty::Bil8 | Ty::Nat | Ty::Nat8 | Ty::Huruf | Ty::Log | Ty::Kosong => true,
        Ty::Array(d, _) => boleh_verifikasi(d),
        Ty::Irisan(d) => boleh_verifikasi(d),
        _ => false,
    }
}

fn sort(t: &Ty) -> &'static str {
    match t {
        Ty::Log => "Bool",
        Ty::Array(d, _) => match &**d {
            Ty::Log => "Bool",
            _ => "(Array Int Int)",
        },
        Ty::Irisan(d) => match &**d {
            Ty::Log => "Bool",
            _ => "(Array Int Int)",
        },
        _ => "Int",
    }
}


fn bool_ty(t: &Ty) -> bool {
    matches!(t, Ty::Log)
}

impl<'a> Vc<'a> {
    fn spek_mulai(&mut self) -> bool {
        let lama = self.spek;
        self.spek = true;
        lama
    }

    fn spek_selesai(&mut self, lama: bool) {
        self.spek = lama;
    }

    fn segar(&mut self, nama: &str, ty: &Ty) -> String {
        let v = self.versi.entry(nama.to_string()).or_insert(0);
        *v += 1;
        let bersih: String = nama.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
        let s = format!("{}_{}", bersih, *v);
        let srt = sort(ty);
        self.decl.push(format!("(declare-fun {} () {})", s, srt));
        if matches!(ty, Ty::Array(_, _) | Ty::Irisan(_)) && srt == "(Array Int Int)" {
            let l = format!("{}_len", s);
            self.decl.push(format!("(declare-fun {} () Int)", l));
        }
        s
    }

    fn panj(&self, n: Nl) -> String {
        match &n.panjang {
            Some(p) => p.clone(),
            None => "0".into(),
        }
    }

    fn tugas(&mut self, deskripsi: String, baris: u32, harus: String, asumsi: &[String]) {
        let skrip = self.skrip(&harus, asumsi);
        let (hasil, model) = jalankan_z3(&skrip);
        self.tugas.push(Tugas { deskripsi, baris, hasil, model });
    }

    fn skrip(&self, harus: &str, asumsi: &[String]) -> String {
        let mut s = String::new();
        s.push_str("(set-logic ALL)\n");
        s.push_str("(set-option :produce-models true)\n");
        s.push_str("(set-option :timeout 8000)\n");
        s.push_str("(set-option :smt.arith.solver 6)\n");
        for d in &self.decl {
            s.push_str(d);
            s.push('\n');
        }
        let mut pre = asumsi.to_vec();
        pre.retain(|x| x != "true");
        if pre.is_empty() {
            s.push_str("(assert (not ");
            s.push_str(harus);
            s.push_str("))\n");
        } else {
            s.push_str("(assert (not (=> (and ");
            for a in &pre {
                s.push_str(a);
                s.push(' ');
            }
            s.push_str(") ");
            s.push_str(harus);
            s.push_str(")))\n");
        }
        s.push_str("(check-sat)\n(get-model)\n");
        s
    }

    fn jalankan(&mut self) -> HasilFungsi {
        let f = self.fungsi;
        let mut env = Env::new();
        let mut pc: Vec<String> = Vec::new();
        for p in &f.params {
            let s = self.segar(&p.nama, &p.ty);
            let panjang = if matches!(p.ty, Ty::Array(_, _) | Ty::Irisan(_)) {
                Some(format!("{}_len", s))
            } else {
                None
            };
            if let Some(l) = &panjang {
                pc.push(format!("(and (>= {} 0) (<= {} 9223372036854775807))", l, l));
            }
            if let Some(d) = domain(&p.ty, &s) {
                pc.push(d);
            }
            env.insert(p.nama.clone(), Nl { term: s, ty: p.ty.clone(), panjang });
        }
        let mut j = Jalur { env, pc, hidup: true };
        if ada_lompat(&f.badan) {
            return HasilFungsi {
                nama: f.nama.clone(),
                tugas: vec![],
                dilewati: Some("memakai `henti`/`lanjut`; belum dimodelkan".into()),
                catatan: vec![],
            };
        }
        let perlu = f.perlu.clone();
        let lama_spek = self.spek_mulai();
        for e in &perlu {
            let n = self.eks(e, &mut j);
            j.pc.push(n.term);
        }
        self.spek_selesai(lama_spek);
        let badan = f.badan.clone();
        self.blok(&badan, &mut j);
        HasilFungsi {
            nama: f.nama.clone(),
            tugas: std::mem::take(&mut self.tugas),
            dilewati: None,
            catatan: std::mem::take(&mut self.catatan),
        }
    }

    fn blok(&mut self, b: &Block, j: &mut Jalur) {
        for s in &b.stmts {
            if !j.hidup {
                return;
            }
            self.stmt(s, j);
        }
    }

    fn stmt(&mut self, st: &Stmt, j: &mut Jalur) {
        match st {
            Stmt::Ikat { nama, nilai, .. } => {
                let n = self.eks(nilai, j);
                j.env.insert(nama.clone(), n);
            }
            Stmt::Tugas { target, nilai, .. } => match &target.kind {
                ExprKind::Var(nama) => {
                    let n = self.eks(nilai, j);
                    j.env.insert(nama.clone(), n);
                }
                ExprKind::Indeks { dasar, indeks } => {
                    if let ExprKind::Var(nama) = &dasar.kind {
                        let dasar_n = match j.env.get(nama).cloned() {
                            Some(x) => x,
                            None => return,
                        };
                        let i = self.eks(indeks, j);
                        let v = self.eks(nilai, j);
                        let len = self.panj(dasar_n.clone());
                        let pc = j.pc.clone();
                        self.tugas(
                            format!("batas indeks pada penugasan `{}[...]`", nama),
                            target.span.baris,
                            format!("(and (<= 0 {}) (< {} {}))", i.term, i.term, len),
                            &pc,
                        );
                        let baru = Nl {
                            term: format!("(store {} {} {})", dasar_n.term, i.term, v.term),
                            ty: dasar_n.ty.clone(),
                            panjang: dasar_n.panjang.clone(),
                        };
                        j.env.insert(nama.clone(), baru);
                    }
                }
                _ => {
                    self.eks(nilai, j);
                }
            },
            Stmt::Jika { kond, maka, lain, .. } => {
                let k = self.eks(kond, j);
                let mut a = j.clone();
                a.pc.push(k.term.clone());
                self.blok(maka, &mut a);
                let mut b = j.clone();
                b.pc.push(format!("(not {})", k.term));
                if let Some(l) = lain {
                    self.blok(l, &mut b);
                }
                *j = self.gabung(&k.term, a, b);
            }
            Stmt::Selama { kond, invarian, badan, .. } => {
                let lama_spek = self.spek_mulai();
                let inv: Vec<String> = invarian.iter().map(|e| self.eks(e, j).term).collect();
                self.spek_selesai(lama_spek);
                let pc0 = j.pc.clone();
                for i in &inv {
                    self.tugas("invarian berlaku saat masuk loop".into(), st_span(st).baris, i.clone(), &pc0);
                }
                let diubah = ditugaskan(badan);
                let mut h = j.clone();
                for nama in &diubah {
                    if let Some(lama) = h.env.get(nama).cloned() {
                        let baru = self.segar(nama, &lama.ty);
                        let panjang = if matches!(lama.ty, Ty::Array(_, _) | Ty::Irisan(_)) {
                            let l = self.segar(&format!("{}__len", nama), &Ty::Bil);
                            let lama_panjang = self.panj(lama.clone());
                            h.pc.push(format!("(= {} {})", l, lama_panjang));
                            Some(l)
                        } else {
                            None
                        };
                        h.env.insert(nama.clone(), Nl { term: baru, ty: lama.ty.clone(), panjang });
                    }
                }
                let lama_spek = self.spek_mulai();
                let inv_h: Vec<String> = invarian.iter().map(|e| self.eks(e, &mut h).term).collect();
                self.spek_selesai(lama_spek);
                for i in &inv_h {
                    h.pc.push(i.clone());
                }
                let mut bj = h.clone();
                let k = self.eks(kond, &mut bj);
                bj.pc.push(k.term.clone());
                self.blok(badan, &mut bj);
                let lama_spek = self.spek_mulai();
                let inv2: Vec<String> = invarian.iter().map(|e| self.eks(e, &mut bj.clone()).term).collect();
                self.spek_selesai(lama_spek);
                for i in &inv2 {
                    self.tugas("invarian dipertahankan oleh badan loop".into(), st_span(st).baris, i.clone(), &bj.pc);
                }
                let mut keluar_ = h;
                keluar_.pc.push(format!("(not {})", k.term));
                for i in &inv_h {
                    keluar_.pc.push(i.clone());
                }
                *j = keluar_;
            }
            Stmt::UntukRentang { nama, dari, sampai, inklusif, badan, span } => {
                let a = self.eks(dari, j);
                let b = self.eks(sampai, j);
                let awal = self.segar(&format!("{}_awal", nama), &Ty::Bil);
                let akhir = self.segar(&format!("{}_akhir", nama), &Ty::Bil);
                let mut h = j.clone();
                h.pc.push(format!("(= {} {})", awal, a.term));
                h.pc.push(format!("(= {} {})", akhir, b.term));
                let i_sym = self.segar(nama, &Ty::Bil);
                h.env.insert(nama.clone(), Nl { term: i_sym.clone(), ty: Ty::Bil, panjang: None });
                let batas = if *inklusif { format!("(< {} (+ {} 1))", i_sym, akhir) } else { format!("(< {} {})", i_sym, akhir) };
                let inv = vec![format!("(<= {} {})", awal, i_sym), batas];
                for i in &inv {
                    h.pc.push(i.clone());
                }
                let mut bj = h.clone();
                let kond = if *inklusif { format!("(<= {} {})", i_sym, akhir) } else { format!("(< {} {})", i_sym, akhir) };
                bj.pc.push(kond.clone());
                self.blok(badan, &mut bj);
                for i in &inv {
                    self.tugas("invarian `untuk` dipertahankan".into(), span.baris, i.clone(), &bj.pc);
                }
                let mut keluar_ = h;
                keluar_.pc.push(format!("(not {})", kond));
                for i in &inv {
                    keluar_.pc.push(i.clone());
                }
                *j = keluar_;
            }
            Stmt::UntukIsi { nama, wadah, badan, .. } => {
                let w = self.eks(wadah, j);
                let len = self.panj(w.clone());
                let i_sym = self.segar(&format!("{}_i", nama), &Ty::Bil);
                let mut h = j.clone();
                h.pc.push(format!("(<= 0 {})", i_sym));
                h.pc.push(format!("(< {} {})", i_sym, len));
                let el = Nl { term: format!("(select {} {})", w.term, i_sym), ty: elemen(&w.ty), panjang: None };
                h.env.insert(nama.clone(), el);
                let mut bj = h.clone();
                bj.pc.push(format!("(< {} {})", i_sym, len));
                self.blok(badan, &mut bj);
                let mut keluar_ = h;
                keluar_.pc.push(format!("(>= {} {})", i_sym, len));
                *j = keluar_;
            }
            Stmt::Kembali(nilai, _) => {
                if let Some(e) = nilai {
                    let n = self.eks(e, j);
                    if !yg_bulat(&n.ty) && !bool_ty(&n.ty) {
                        j.hidup = false;
                        return;
                    }
                    let jamin = self.fungsi.jamin.clone();
                    if !jamin.is_empty() {
                        let mut jj = j.clone();
                        jj.env.insert("hasil".into(), n.clone());
                        let lama_spek = self.spek_mulai();
                        for c in &jamin {
                            let t = self.eks(c, &mut jj);
                            let pc = jj.pc.clone();
                            self.tugas("jamin (postcondition) pada `kembali`".into(), e.span.baris, t.term, &pc);
                        }
                        self.spek_selesai(lama_spek);
                    }
                }
                j.hidup = false;
            }
            Stmt::Tegas { kond, .. } => {
                let k = self.eks(kond, j);
                let pc = j.pc.clone();
                self.tugas("`tegas` harus selalu benar".into(), st_span(st).baris, k.term, &pc);
            }
            Stmt::Ekspresi(e) => {
                self.eks(e, j);
                if let ExprKind::Panggil { nama, .. } = &e.kind {
                    if nama == "gagal" {
                        j.hidup = false;
                    }
                }
            }
            Stmt::Henti(_) | Stmt::Lanjut(_) => {
                j.hidup = false;
            }
            Stmt::Blok(b) => self.blok(b, j),
        }
    }

    fn gabung(&mut self, k: &str, a: Jalur, b: Jalur) -> Jalur {
        match (a.hidup, b.hidup) {
            (true, false) => a,
            (false, true) => b,
            (false, false) => Jalur { env: a.env, pc: a.pc, hidup: false },
            (true, true) => {
                let mut env = Env::new();
                for (nama, na) in &a.env {
                    if let Some(nb) = b.env.get(nama) {
                        if na.term == nb.term {
                            env.insert(nama.clone(), na.clone());
                        } else {
                            env.insert(nama.clone(), Nl {
                                term: format!("(ite {} {} {})", k, na.term, nb.term),
                                ty: na.ty.clone(),
                                panjang: na.panjang.clone(),
                            });
                        }
                    } else {
                        env.insert(nama.clone(), na.clone());
                    }
                }
                let mut pc = a.pc.clone();
                pc.retain(|x| b.pc.contains(x));
                Jalur { env, pc, hidup: true }
            }
        }
    }

    fn eks(&mut self, e: &Expr, j: &mut Jalur) -> Nl {
        let t = self.sema.tipe.get(&e.id).cloned().unwrap_or(Ty::Bil);
        match &e.kind {
            ExprKind::Bil(v) => Nl { term: num(*v as i128), ty: t, panjang: None },
            ExprKind::Huruf(v) => Nl { term: v.to_string(), ty: Ty::Huruf, panjang: None },
            ExprKind::Log(b) => Nl { term: b.to_string(), ty: Ty::Log, panjang: None },
            ExprKind::Pecah(_) => Nl { term: "0".into(), ty: Ty::Pecah, panjang: None },
            ExprKind::Teks(_) => Nl { term: "0".into(), ty: Ty::Teks, panjang: None },
            ExprKind::Var(n) => match j.env.get(n) {
                Some(x) => x.clone(),
                None => Nl { term: "0".into(), ty: t, panjang: None },
            },
            ExprKind::Biner(op, a, b) => {
                if bool_ty(&t) {
                    let x = self.eks(a, j);
                    let y = self.eks(b, j);
                    let ins = match op {
                        BinOp::Dan => "and",
                        BinOp::Atau => "or",
                        BinOp::Sama => "=",
                        BinOp::TidakSama => "distinct",
                        BinOp::Kecil => "<",
                        BinOp::KecilSama => "<=",
                        BinOp::Besar => ">",
                        BinOp::BesarSama => ">=",
                        _ => "=",
                    };
                    return Nl { term: format!("({} {} {})", ins, x.term, y.term), ty: Ty::Log, panjang: None };
                }
                let x = self.eks(a, j);
                let y = self.eks(b, j);
                let pc = j.pc.clone();
                match op {
                    BinOp::Tambah | BinOp::Kurang | BinOp::Kali => {
                        let ins = match op {
                            BinOp::Tambah => "+",
                            BinOp::Kurang => "-",
                            _ => "*",
                        };
                        let res = format!("({} {} {})", ins, x.term, y.term);
                        let (lo, hi) = batas128(&x.ty);
                        if self.spek { return Nl { term: res, ty: x.ty, panjang: None }; }
                        self.tugas(
                            format!("tidak luber pada `{}`", op.simbol()),
                            e.span.baris,
                            format!("(and (<= {} {}) (<= {} {}))", num(lo), res, res, num(hi)),
                            &pc,
                        );
                        Nl { term: res, ty: x.ty, panjang: None }
                    }
                    BinOp::Bagi | BinOp::Sisa => {
                        if !self.spek {
                            self.tugas("pembagi tidak nol".into(), e.span.baris, format!("(not (= {} 0))", y.term), &pc);
                        }
                        let q = bagi_t(&x.term, &y.term);
                        let res = if *op == BinOp::Bagi { q } else { format!("(- {} (* {} {}))", x.term, y.term, q) };
                        Nl { term: res, ty: x.ty, panjang: None }
                    }
                    _ => Nl { term: "0".into(), ty: t, panjang: None },
                }
            }
            ExprKind::Uner(op, a) => {
                let x = self.eks(a, j);
                match op {
                    UnOp::Bukan => Nl { term: format!("(not {})", x.term), ty: Ty::Log, panjang: None },
                    UnOp::Neg => {
                        let res = format!("(- 0 {})", x.term);
                        let (lo, hi) = batas128(&x.ty);
                        if self.spek { return Nl { term: res, ty: x.ty, panjang: None }; }
                        let pc = j.pc.clone();
                        self.tugas(
                            "tidak luber pada `-`".into(),
                            e.span.baris,
                            format!("(and (<= {} {}) (<= {} {}))", num(lo), res, res, num(hi)),
                            &pc,
                        );
                        Nl { term: res, ty: x.ty, panjang: None }
                    }
                }
            }
            ExprKind::Indeks { dasar, indeks } => {
                let d = self.eks(dasar, j);
                if !matches!(d.ty, Ty::Array(_, _) | Ty::Irisan(_)) {
                    return Nl { term: "0".into(), ty: t, panjang: None };
                }
                let i = self.eks(indeks, j);
                let len = self.panj(d.clone());
                if self.spek {
                    return Nl { term: format!("(select {} {})", d.term, i.term), ty: elemen(&d.ty), panjang: None };
                }
                let pc = j.pc.clone();
                self.tugas(
                    format!("indeks dalam batas (0 <= i < {})", len),
                    e.span.baris,
                    format!("(and (<= 0 {}) (< {} {}))", i.term, i.term, len),
                    &pc,
                );
                Nl { term: format!("(select {} {})", d.term, i.term), ty: elemen(&d.ty), panjang: None }
            }
            ExprKind::Sebagai { nilai, ty } => {
                let x = self.eks(nilai, j);
                if self.spek {
                    return Nl { term: x.term, ty: ty.clone(), panjang: None };
                }
                let (lo, hi) = batas128(ty);
                let pc = j.pc.clone();
                self.tugas(
                    format!("nilai muat saat `sebagai {}`", ty.sebutan()),
                    e.span.baris,
                    format!("(and (<= {} {}) (<= {} {}))", num(lo), x.term, x.term, num(hi)),
                    &pc,
                );
                Nl { term: x.term, ty: ty.clone(), panjang: None }
            }
            ExprKind::ArrayLit(isi) => {
                let ty = t.clone();
                let nama = self.segar("larik", &ty);
                let panjang = match &ty {
                    Ty::Array(_, n) => n.to_string(),
                    _ => "0".into(),
                };
                for (i, x) in isi.iter().enumerate() {
                    let v = self.eks(x, j);
                    j.pc.push(format!("(= (select {} {}) {})", nama, i, v.term));
                }
                Nl { term: nama, ty, panjang: Some(panjang) }
            }
            ExprKind::StrukLit { .. } | ExprKind::Medan { .. } | ExprKind::Varian { .. } | ExprKind::Alamat { .. } => {
                Nl { term: "0".into(), ty: t, panjang: None }
            }
            ExprKind::Panggil { nama, args } => self.panggil_eks(e, nama, args, j),
        }
    }

    fn panggil_eks(&mut self, e: &Expr, nama: &str, args: &[Expr], j: &mut Jalur) -> Nl {
        let hasil_ty = self.sema.tipe.get(&e.id).cloned().unwrap_or(Ty::Kosong);
        let jenis = self.sema.panggil.get(&e.id).cloned();
        if nama == "gagal" {
            self.eks(&args[0], j);
            j.hidup = false;
            return Nl { term: "0".into(), ty: Ty::Kosong, panjang: None };
        }
        if let Some(Panggilan::Bawaan(_)) = jenis {
            match nama {
                "panjang" => {
                    let a = self.eks(&args[0], j);
                    let l = self.panj(a);
                    return Nl { term: l, ty: Ty::Bil, panjang: None };
                }
                "ambil" => {
                    let a = self.eks(&args[0], j);
                    let i = self.eks(&args[1], j);
                    let len = self.panj(a.clone());
                    let pc = j.pc.clone();
                    self.tugas(
                        "indeks `ambil` di dalam batas".into(),
                        e.span.baris,
                        format!("(and (<= 0 {}) (< {} {}))", i.term, i.term, len),
                        &pc,
                    );
                    return Nl { term: format!("(select {} {})", a.term, i.term), ty: elemen(&a.ty), panjang: None };
                }
                _ => {
                    for a in args {
                        self.eks(a, j);
                    }
                    return Nl { term: "0".into(), ty: hasil_ty, panjang: None };
                }
            }
        }
        let sig = self.sema.sig.get(nama).cloned();
        let mut argv_nl = Vec::new();
        for a in args {
            argv_nl.push(self.eks(a, j));
        }
        let argv: Vec<String> = argv_nl.iter().map(|x| x.term.clone()).collect();
        let _ = &argv;
        let (params, hasil, bisa) = match (&sig, self.prog.fungsi.iter().find(|f| &f.nama == nama)) {
            (Some(s), Some(f)) => (
                s.params.clone(),
                s.hasil.clone(),
                boleh_verifikasi(&s.hasil)
                    && s.params.iter().all(boleh_verifikasi)
                    && f.params.iter().all(|p| boleh_verifikasi(&p.ty))
                    && !ada_lompat(&f.badan),
            ),
            (Some(s), None) => (s.params.clone(), s.hasil.clone(), false),
            (None, _) => (vec![], hasil_ty.clone(), false),
        };
        let fnama = format!("f_{}_{}", nama, params.len());
        let deklarasi = format!("(declare-fun {} (", fnama);
        if !self.decl.iter().any(|d| d.starts_with(&deklarasi)) {
            let sargs: Vec<&str> = params.iter().map(|p| if bool_ty(p) { "Bool" } else { "Int" }).collect();
            let shasil = if bool_ty(&hasil) { "Bool" } else { "Int" };
            let argsrt = if sargs.is_empty() { String::new() } else { format!(" ({})", sargs.join(" ")) };
            self.decl.push(format!("(declare-fun {}{} {})", fnama, argsrt, shasil));
        }
        let res = self.segar(&format!("{}_hasil", nama), &hasil);
        if bisa {
            let mut jj = j.clone();
            if let Some(f) = self.prog.fungsi.iter().find(|f| f.nama == *nama) {
                for (p, v) in f.params.iter().zip(argv_nl.iter()) {
                    jj.env.insert(p.nama.clone(), Nl { term: v.term.clone(), ty: p.ty.clone(), panjang: v.panjang.clone() });
                }
                jj.env.insert("hasil".into(), Nl { term: res.clone(), ty: hasil.clone(), panjang: None });
                let perlu = f.perlu.clone();
                let jamin = f.jamin.clone();
                let lama_spek = self.spek_mulai();
                for e in &perlu {
                    let t = self.eks(e, &mut jj);
                    let pc = jj.pc.clone();
                    self.tugas(format!("prasyarat `{}` saat dipanggil", nama), e.span.baris, t.term, &pc);
                }
                for e in &jamin {
                    let t = self.eks(e, &mut jj);
                    j.pc.push(t.term);
                }
                self.spek_selesai(lama_spek);
            }
        }
        Nl { term: res, ty: hasil, panjang: None }
    }
}

#[derive(Clone)]
struct Jalur {
    env: Env,
    pc: Vec<String>,
    hidup: bool,
}

fn elemen(t: &Ty) -> Ty {
    match t {
        Ty::Array(d, _) | Ty::Irisan(d) | Ty::Daftar(d) => (**d).clone(),
        _ => Ty::Bil,
    }
}

fn st_span(st: &Stmt) -> Span {
    match st {
        Stmt::Jika { span, .. }
        | Stmt::Selama { span, .. }
        | Stmt::UntukRentang { span, .. }
        | Stmt::UntukIsi { span, .. }
        | Stmt::Ikat { span, .. }
        | Stmt::Tugas { span, .. }
        | Stmt::Tegas { span, .. }
        | Stmt::Henti(span)
        | Stmt::Lanjut(span)
        | Stmt::Kembali(_, span) => *span,
        Stmt::Ekspresi(e) => e.span,
        Stmt::Blok(b) => b.span,
    }
}

fn num(v: i128) -> String {
    if v < 0 {
        format!("(- {})", -(v as i128))
    } else {
        v.to_string()
    }
}

fn batas128(t: &Ty) -> (i128, i128) {
    match t {
        Ty::Bil => (i64::MIN as i128, i64::MAX as i128),
        Ty::Bil32 => (i32::MIN as i128, i32::MAX as i128),
        Ty::Bil8 => (i8::MIN as i128, i8::MAX as i128),
        Ty::Nat => (0, u64::MAX as i128),
        Ty::Nat8 => (0, u8::MAX as i128),
        Ty::Huruf => (0, u8::MAX as i128),
        Ty::Log => (0, 1),
        _ => (i64::MIN as i128, i64::MAX as i128),
    }
}

fn domain(t: &Ty, term: &str) -> Option<String> {
    if yg_bulat(t) {
        let (lo, hi) = batas128(t);
        Some(format!("(and (<= {} {}) (<= {} {}))", num(lo), term, term, num(hi)))
    } else {
        None
    }
}

fn yg_bulat(t: &Ty) -> bool {
    matches!(t, Ty::Bil | Ty::Bil32 | Ty::Bil8 | Ty::Nat | Ty::Nat8 | Ty::Huruf)
}

fn bagi_t(a: &str, b: &str) -> String {
    format!(
        "(ite (>= {a} 0) (ite (>= {b} 0) (div {a} {b}) (- (div {a} (- {b})))) (ite (>= {b} 0) (- (div (- {a}) {b})) (div (- {a}) (- {b}))))",
        a = a,
        b = b
    )
}

fn ditugaskan(b: &Block) -> HashSet<String> {
    let mut s = HashSet::new();
    kumpul_tugas(b, &mut s);
    s
}

fn kumpul_tugas(b: &Block, s: &mut HashSet<String>) {
    for st in &b.stmts {
        match st {
            Stmt::Tugas { target, .. } => match &target.kind {
                ExprKind::Var(n) => {
                    s.insert(n.clone());
                }
                ExprKind::Indeks { dasar, .. } => {
                    if let ExprKind::Var(n) = &dasar.kind {
                        s.insert(n.clone());
                    }
                }
                _ => {}
            },
            Stmt::Jika { maka, lain, .. } => {
                kumpul_tugas(maka, s);
                if let Some(l) = lain {
                    kumpul_tugas(l, s);
                }
            }
            Stmt::Selama { badan, .. } | Stmt::UntukRentang { badan, .. } | Stmt::UntukIsi { badan, .. } => {
                kumpul_tugas(badan, s);
            }
            Stmt::Blok(b) => kumpul_tugas(b, s),
            _ => {}
        }
    }
}

fn ada_lompat(b: &Block) -> bool {
    b.stmts.iter().any(|s| match s {
        Stmt::Henti(_) | Stmt::Lanjut(_) => true,
        Stmt::Jika { maka, lain, .. } => ada_lompat(maka) || lain.as_ref().map(ada_lompat).unwrap_or(false),
        Stmt::Selama { badan, .. } | Stmt::UntukRentang { badan, .. } | Stmt::UntukIsi { badan, .. } => ada_lompat(badan),
        Stmt::Blok(b) => ada_lompat(b),
        _ => false,
    })
}

fn jalankan_z3(skrip: &str) -> (Hasil, Option<String>) {
    let tmp = std::env::temp_dir().join(format!("nusa-vc-{}-{}.smt2", std::process::id(), counter()));
    if std::env::var("NUSA_SMT").is_ok() {
        let dbg = std::env::temp_dir().join(format!("nusa-dbg-{}.smt2", counter()));
        let _ = std::fs::write(&dbg, skrip);
    }
    if std::fs::write(&tmp, skrip).is_err() {
        return (Hasil::TidakDiketahui, None);
    }
    let anak = Command::new("z3")
        .arg("-smt2")
        .arg(&tmp)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let _ = std::fs::remove_file(&tmp);
    match anak {
        Ok(keluaran) => {
            let teks = String::from_utf8_lossy(&keluaran.stdout).to_string();
            let baris1 = teks.lines().next().unwrap_or("").trim().to_string();
            if baris1 == "unsat" {
                (Hasil::Terbukti, None)
            } else if baris1 == "sat" {
                let model: String = teks
                    .lines()
                    .skip(1)
                    .take(16)
                    .collect::<Vec<_>>()
                    .join(" ")
                    .replace("(define-fun ", "")
                    .replace(" () ", " = ")
                    .replace(['(', ')'], " ");
                (Hasil::Gagal, Some(model.trim().to_string()))
            } else {
                (Hasil::TidakDiketahui, Some(teks.lines().take(3).collect::<Vec<_>>().join(" ")))
            }
        }
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "nusa: tidak dapat menjalankan z3: {}", e);
            (Hasil::TidakDiketahui, None)
        }
    }
}

fn counter() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}
