//! Pembangkit kode LLVM IR (teks) untuk NUSA.
//! Tanpa pustaka LLVM di sisi kompiler: berkas .ll dihasilkan lalu
//! dirakit oleh `llc`/`clang`. Kompiler tetap kecil dan cepat dibangun.

use crate::ast::*;
use crate::sema::{Panggilan, Sema};
use std::collections::{BTreeMap, HashMap};

pub struct Opsi {
    pub jaga: bool,      // sisipkan pemeriksaan runtime (batas, luber, bagi nol)
    pub opt: &'static str,
}

#[derive(Clone, Debug)]
struct Val {
    reg: String,
    ty: Ty,
}

#[derive(Clone, Debug)]
struct Lokal {
    ptr: String,
    ty: Ty,
    daftar: bool,
}

pub struct Codegen<'a> {
    prog: &'a Prog,
    sema: &'a Sema,
    #[allow(dead_code)]
    pub out: String,
    teks: Vec<(String, Vec<u8>)>,
    teks_idx: HashMap<Vec<u8>, String>,
    daftar: BTreeMap<String, Ty>,
    irisan: BTreeMap<String, Ty>,
    tmp: u32,
    label: u32,
    opsi: Opsi,
    fns: Vec<String>,
    ctx: FnCtx,
    varian: BTreeMap<String, String>,
    takaman: u32,
}

struct FnCtx {
    alloca: String,
    badan: String,
    scope: Vec<HashMap<String, Lokal>>,
    buntu: bool,
    loop_stack: Vec<(String, String)>,
    jangan_pindah: u32,
    hasil: Ty,
    nama: String,
}

impl FnCtx {
    fn kosong() -> Self {
        FnCtx {
            alloca: String::new(),
            badan: String::new(),
            scope: vec![HashMap::new()],
            buntu: false,
            loop_stack: Vec::new(),
            jangan_pindah: 0,
            hasil: Ty::Kosong,
            nama: String::new(),
        }
    }
}

impl<'a> Codegen<'a> {
    fn emit(&mut self, s: &str) {
        if self.ctx.buntu {
            let l = self.lbl("buntu");
            self.ctx.badan.push_str(&format!("{}:\n", l));
            self.ctx.buntu = false;
        }
        self.ctx.badan.push_str("  ");
        self.ctx.badan.push_str(s);
        self.ctx.badan.push('\n');
    }

    fn alloca(&mut self, ty: &str) -> String {
        let n = self.tmp();
        let nm = format!("%a{}", &n[2..]);
        self.ctx.alloca.push_str(&format!("  {} = alloca {}\n", nm, ty));
        nm
    }

    fn muat(&mut self, ptr: &str, ty: &Ty) -> String {
        let t = self.llty(ty);
        let n = self.tmp();
        self.emit(&format!("{} = load {}, {}* {}", n, t, t, ptr));
        n
    }

    fn simpan(&mut self, ptr: &str, ty: &Ty, val: &str) {
        let t = self.llty(ty);
        self.emit(&format!("store {} {}, {}* {}", t, val, t, ptr));
    }

    fn masuk(&mut self) {
        self.ctx.scope.push(HashMap::new());
    }

    /// Tipe yang memerlukan pembebasan otomatis di akhir cakupan.
    fn perlu_bebas(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Daftar(_) => true,
            Ty::Nama(n) => self
                .sema
                .struk
                .get(n)
                .map(|s| s.medan.iter().any(|(_, t)| self.perlu_bebas(t)))
                .unwrap_or(false),
            _ => false,
        }
    }

    fn bebas_nilai(&mut self, reg: &str, ty: &Ty) {
        match ty {
            Ty::Daftar(_) => {
                let t = self.llty(ty);
                let p = self.tmp();
                self.emit(&format!("{} = extractvalue {} {}, 0", p, t, reg));
                self.emit(&format!("call void @nusa_daftar_bebas(i8* {})", p));
            }
            Ty::Nama(n) => {
                let medan = match self.sema.struk.get(n) {
                    Some(s) => s.medan.clone(),
                    None => return,
                };
                let t = self.llty(ty);
                for (i, (_, fty)) in medan.iter().enumerate() {
                    if self.perlu_bebas(fty) {
                        let p = self.tmp();
                        self.emit(&format!("{} = extractvalue {} {}, {}", p, t, reg, i));
                        self.bebas_nilai(&p, fty);
                    }
                }
            }
            _ => {}
        }
    }

    fn keluar(&mut self) {
        let skop = self.ctx.scope.pop().unwrap_or_default();
        let mut nama: Vec<&String> = skop.keys().collect();
        nama.sort();
        for n in nama {
            if let Some(l) = skop.get(n) {
                if l.daftar {
                    let v = self.muat(&l.ptr, &l.ty);
                    self.bebas_nilai(&v, &l.ty.clone());
                }
            }
        }
    }

    fn cari(&self, nama: &str) -> Option<Lokal> {
        for s in self.ctx.scope.iter().rev() {
            if let Some(l) = s.get(nama) {
                return Some(l.clone());
            }
        }
        None
    }

    fn ikat(&mut self, nama: &str, l: Lokal) {
        self.ctx.scope.last_mut().unwrap().insert(nama.to_string(), l);
    }

    pub fn baru(prog: &'a Prog, sema: &'a Sema, opsi: Opsi) -> Self {
        Codegen {
            prog,
            sema,
            out: String::new(),
            teks: Vec::new(),
            teks_idx: HashMap::new(),
            daftar: BTreeMap::new(),
            irisan: BTreeMap::new(),
            tmp: 0,
            label: 0,
            opsi,
            fns: Vec::new(),
            ctx: FnCtx::kosong(),
            varian: sema
                .enumerasi
                .keys()
                .map(|k| (k.clone(), format!("@.varian.{}", k)))
                .collect(),
            takaman: 0,
        }
    }

    fn tmp(&mut self) -> String {
        self.tmp += 1;
        format!("%t{}", self.tmp)
    }

    fn lbl(&mut self, dasar: &str) -> String {
        self.label += 1;
        format!("{}{}", dasar, self.label)
    }

    fn mangle(&self, t: &Ty) -> String {
        match t {
            Ty::Bil => "bil".into(),
            Ty::Bil32 => "bil32".into(),
            Ty::Bil8 => "bil8".into(),
            Ty::Nat => "nat".into(),
            Ty::Nat8 => "nat8".into(),
            Ty::Pecah => "pc".into(),
            Ty::Log => "lg".into(),
            Ty::Huruf => "hr".into(),
            Ty::Teks => "tk".into(),
            Ty::Kosong => "ks".into(),
            Ty::Array(d, n) => format!("a{}_{}", n, self.mangle(d)),
            Ty::Irisan(d) => format!("i_{}", self.mangle(d)),
            Ty::Ref { ubah, dalam } => format!("r{}_{}", if *ubah { "u" } else { "s" }, self.mangle(dalam)),
            Ty::Daftar(d) => format!("d_{}", self.mangle(d)),
            Ty::Nama(n) => format!("n_{}", n),
        }
    }

    fn llty(&mut self, t: &Ty) -> String {
        match t {
            Ty::Bil | Ty::Nat => "i64".into(),
            Ty::Bil32 => "i32".into(),
            Ty::Bil8 | Ty::Nat8 | Ty::Huruf => "i8".into(),
            Ty::Pecah => "double".into(),
            Ty::Log => "i1".into(),
            Ty::Kosong => "void".into(),
            Ty::Teks => "%teks".into(),
            Ty::Array(d, n) => {
                let d = self.llty(d);
                format!("[{} x {}]", n, d)
            }
            Ty::Irisan(d) => {
                let m = self.mangle(&Ty::Irisan(d.clone()));
                self.irisan.insert(m.clone(), (**d).clone());
                format!("%irisan.{}", m)
            }
            Ty::Daftar(d) => {
                let m = self.mangle(&Ty::Daftar(d.clone()));
                self.daftar.insert(m.clone(), (**d).clone());
                format!("%daftar.{}", m)
            }
            Ty::Ref { dalam, .. } => {
                let d = self.llty(dalam);
                format!("{}*", d)
            }
            Ty::Nama(n) => {
                if self.sema.enumerasi.contains_key(n) {
                    "i32".into()
                } else {
                    format!("%struk.{}", n)
                }
            }
        }
    }

    fn ukuran(&mut self, t: &Ty) -> u64 {
        match t {
            Ty::Bil | Ty::Nat | Ty::Pecah | Ty::Teks | Ty::Ref { .. } | Ty::Irisan(_) => 8,
            Ty::Bil32 => 4,
            Ty::Bil8 | Ty::Nat8 | Ty::Huruf | Ty::Log => 1,
            Ty::Kosong => 0,
            Ty::Array(d, n) => self.ukuran(d) * (*n as u64),
            Ty::Daftar(_) => 24,
            Ty::Nama(n) => {
                if self.sema.enumerasi.contains_key(n) {
                    4
                } else {
                    self.sema.struk.get(n).map(|s| s.medan.iter().map(|(_, m)| self.ukuran(m)).sum()).unwrap_or(0)
                }
            }
        }
    }

    fn teks_konst(&mut self, isi: &str) -> String {
        let bytes = isi.as_bytes().to_vec();
        if let Some(n) = self.teks_idx.get(&bytes) {
            return n.clone();
        }
        let nama = format!("@.t{}", self.teks.len());
        self.teks.push((nama.clone(), bytes.clone()));
        self.teks_idx.insert(bytes, nama.clone());
        nama
    }

    fn blok(&mut self, b: &Block) {
        self.masuk();
        for s in &b.stmts {
            self.stmt(s);
        }
        self.keluar();
    }

    fn stmt(&mut self, st: &Stmt) {
        match st {
            Stmt::Ikat { nama, ty, nilai, span, .. } => {
                let mut v = self.ekspresi(nilai);
                if let Some(t) = ty {
                    v = self.ubah_tipe(v, t);
                }
                let llt = self.llty(&v.ty);
                let p = self.alloca(&llt);
                self.simpan(&p, &v.ty, &v.reg);
                let _ = span;
                let daftar = self.perlu_bebas(&v.ty);
                self.ikat(nama, Lokal { ptr: p, ty: v.ty.clone(), daftar });
            }
            Stmt::Tugas { target, nilai, .. } => {
                let t_target = self.sema.tipe.get(&target.id).cloned().unwrap_or(Ty::Bil);
                let mut v = self.ekspresi(nilai);
                v = self.ubah_tipe(v, &t_target);
                let p = self.tempat_ptr(target);
                self.simpan(&p, &t_target, &v.reg);
            }
            Stmt::Jika { kond, maka, lain, .. } => {
                let k = self.ekspresi(kond);
                let lt = self.lbl("jika.maka");
                let le = self.lbl("jika.lain");
                let lm = self.lbl("jika.tamat");
                if lain.is_some() {
                    self.emit(&format!("br i1 {}, label %{}, label %{}", k.reg, lt, le));
                } else {
                    self.emit(&format!("br i1 {}, label %{}, label %{}", k.reg, lt, lm));
                }
                let sebelum = self.ctx.buntu;
                self.ctx.buntu = false;
                self.ctx.badan.push_str(&format!("{}:\n", lt));
                self.blok(maka);
                let buntu_maka = self.ctx.buntu;
                if !buntu_maka {
                    self.emit(&format!("br label %{}", lm));
                }
                if let Some(l) = lain {
                    self.ctx.buntu = false;
                    self.ctx.badan.push_str(&format!("{}:\n", le));
                    self.blok(l);
                    if !self.ctx.buntu {
                        self.emit(&format!("br label %{}", lm));
                    }
                }
                self.ctx.badan.push_str(&format!("{}:\n", lm));
                self.ctx.buntu = sebelum && buntu_maka;
            }
            Stmt::Selama { kond, badan, .. } => {
                let lc = self.lbl("selama.cek");
                let lb = self.lbl("selama.badan");
                let le = self.lbl("selama.tamat");
                self.emit(&format!("br label %{}", lc));
                self.ctx.badan.push_str(&format!("{}:\n", lc));
                self.ctx.buntu = false;
                let k = self.ekspresi(kond);
                self.emit(&format!("br i1 {}, label %{}, label %{}", k.reg, lb, le));
                self.ctx.badan.push_str(&format!("{}:\n", lb));
                self.ctx.buntu = false;
                self.ctx.loop_stack.push((lc.clone(), le.clone()));
                self.blok(badan);
                self.ctx.loop_stack.pop();
                if !self.ctx.buntu {
                    self.emit(&format!("br label %{}", lc));
                }
                self.ctx.badan.push_str(&format!("{}:\n", le));
                self.ctx.buntu = false;
            }
            Stmt::UntukRentang { nama, dari, sampai, inklusif, badan, .. } => {
                let a = self.ekspresi(dari);
                let b = self.ekspresi(sampai);
                let p = self.alloca("i64");
                self.simpan(&p, &Ty::Bil, &a.reg);
                let lc = self.lbl("untuk.cek");
                let lb = self.lbl("untuk.badan");
                let ls = self.lbl("untuk.langkah");
                let le = self.lbl("untuk.tamat");
                self.emit(&format!("br label %{}", lc));
                self.ctx.badan.push_str(&format!("{}:\n", lc));
                self.ctx.buntu = false;
                let i = self.muat(&p, &Ty::Bil);
                let c = self.tmp();
                let op = if *inklusif { "sle" } else { "slt" };
                self.emit(&format!("{} = icmp {} i64 {}, {}", c, op, i, b.reg));
                self.emit(&format!("br i1 {}, label %{}, label %{}", c, lb, le));
                self.ctx.badan.push_str(&format!("{}:\n", lb));
                self.ctx.buntu = false;
                self.masuk();
                self.ikat(nama, Lokal { ptr: p.clone(), ty: Ty::Bil, daftar: false });
                self.ctx.loop_stack.push((ls.clone(), le.clone()));
                for s in &badan.stmts {
                    self.stmt(s);
                }
                self.ctx.loop_stack.pop();
                self.keluar();
                if !self.ctx.buntu {
                    self.emit(&format!("br label %{}", ls));
                }
                self.ctx.badan.push_str(&format!("{}:\n", ls));
                self.ctx.buntu = false;
                let i2 = self.muat(&p, &Ty::Bil);
                let i3 = self.tmp();
                self.emit(&format!("{} = add nuw nsw i64 {}, 1", i3, i2));
                self.simpan(&p, &Ty::Bil, &i3);
                self.emit(&format!("br label %{}", lc));
                self.ctx.badan.push_str(&format!("{}:\n", le));
                self.ctx.buntu = false;
            }
            Stmt::UntukIsi { nama, wadah, badan, span } => {
                let elemen = self.sema.elemen_untuk.get(&(span.baris, span.kol)).cloned().unwrap_or(Ty::Bil);
                let (ptr, len) = self.ptr_dan_panjang(wadah);
                let p = self.alloca("i64");
                self.simpan(&p, &Ty::Bil, "0");
                let lc = self.lbl("isi.cek");
                let lb = self.lbl("isi.badan");
                let ls = self.lbl("isi.langkah");
                let le = self.lbl("isi.tamat");
                self.emit(&format!("br label %{}", lc));
                self.ctx.badan.push_str(&format!("{}:\n", lc));
                self.ctx.buntu = false;
                let i = self.muat(&p, &Ty::Bil);
                let c = self.tmp();
                self.emit(&format!("{} = icmp ult i64 {}, {}", c, i, len));
                self.emit(&format!("br i1 {}, label %{}, label %{}", c, lb, le));
                self.ctx.badan.push_str(&format!("{}:\n", lb));
                self.ctx.buntu = false;
                let et = self.llty(&elemen);
                let ep = self.tmp();
                let i4 = self.muat(&p, &Ty::Bil);
                self.emit(&format!("{} = getelementptr inbounds {}, {}* {}, i64 {}", ep, et, et, ptr, i4));
                let el = self.muat(&ep, &elemen);
                let lp = self.alloca(&et);
                self.simpan(&lp, &elemen, &el);
                self.masuk();
                self.ikat(nama, Lokal { ptr: lp, ty: elemen, daftar: false });
                self.ctx.loop_stack.push((ls.clone(), le.clone()));
                for s in &badan.stmts {
                    self.stmt(s);
                }
                self.ctx.loop_stack.pop();
                self.keluar();
                if !self.ctx.buntu {
                    self.emit(&format!("br label %{}", ls));
                }
                self.ctx.badan.push_str(&format!("{}:\n", ls));
                self.ctx.buntu = false;
                let i5 = self.muat(&p, &Ty::Bil);
                let i6 = self.tmp();
                self.emit(&format!("{} = add nuw nsw i64 {}, 1", i6, i5));
                self.simpan(&p, &Ty::Bil, &i6);
                self.emit(&format!("br label %{}", lc));
                self.ctx.badan.push_str(&format!("{}:\n", le));
                self.ctx.buntu = false;
            }
            Stmt::Kembali(nilai, _) => {
                match nilai {
                    Some(e) => {
                        let v = self.ekspresi_pindah(e);
                        let t = self.llty(&v.ty);
                        self.emit(&format!("ret {} {}", t, v.reg));
                    }
                    None => self.emit("ret void"),
                }
                self.ctx.buntu = true;
            }
            Stmt::Tegas { kond, pesan, span } => {
                let k = self.ekspresi(kond);
                let lo = self.lbl("tegas.ok");
                let lg = self.lbl("tegas.gagal");
                self.emit(&format!("br i1 {}, label %{}, label %{}", k.reg, lo, lg));
                self.ctx.badan.push_str(&format!("{}:\n", lg));
                self.ctx.buntu = false;
                let pesan_teks = pesan.clone().unwrap_or_else(|| "tegas gagal".to_string());
                let p = self.teks_konst(&pesan_teks);
                let l = pesan_teks.len();
                self.emit(&format!("call void @nusa_tegas(i8* {}, i64 {}, i32 {})", p, l, span.baris));
                self.emit("unreachable");
                self.ctx.badan.push_str(&format!("{}:\n", lo));
                self.ctx.buntu = false;
            }
            Stmt::Ekspresi(e) => {
                self.ekspresi(e);
            }
            Stmt::Henti(_) => {
                if let Some((_, le)) = self.ctx.loop_stack.last().cloned() {
                    self.emit(&format!("br label %{}", le));
                    self.ctx.buntu = true;
                }
            }
            Stmt::Lanjut(_) => {
                if let Some((lc, _)) = self.ctx.loop_stack.last().cloned() {
                    self.emit(&format!("br label %{}", lc));
                    self.ctx.buntu = true;
                }
            }
            Stmt::Cocok { nilai, kasus, lainnya, .. } => {
                let v = self.ekspresi(nilai);
                let enum_nama = match self.sema.tipe.get(&nilai.id).cloned() {
                    Some(Ty::Nama(n)) => n,
                    _ => String::new(),
                };
                let akhir = self.lbl("cocok.tamat");
                let bawaan = self.lbl("cocok.lain");
                let mut dispatch = vec![self.lbl("cocok.cek")];
                let mut label_kasus = Vec::new();
                for _ in kasus {
                    dispatch.push(self.lbl("cocok.cek"));
                    label_kasus.push(self.lbl("cocok.kasus"));
                }
                self.emit(&format!("br label %{}", dispatch[0]));
                for (i, (nama, _)) in kasus.iter().enumerate() {
                    self.ctx.badan.push_str(&format!("{}:\n", dispatch[i]));
                    let idx = self
                        .sema
                        .enumerasi
                        .get(&enum_nama)
                        .and_then(|e| e.varian.iter().position(|x| x == nama))
                        .unwrap_or(0);
                    let c = self.tmp();
                    self.emit(&format!("{} = icmp eq i32 {}, {}", c, v.reg, idx));
                    self.emit(&format!("br i1 {}, label %{}, label %{}", c, label_kasus[i], dispatch[i + 1]));
                }
                self.ctx.badan.push_str(&format!("{}:\n", dispatch[kasus.len()]));
                if lainnya.is_some() {
                    self.emit(&format!("br label %{}", bawaan));
                } else {
                    self.emit(&format!("br label %{}", akhir));
                }
                for (i, (_, b)) in kasus.iter().enumerate() {
                    self.ctx.badan.push_str(&format!("{}:\n", label_kasus[i]));
                    self.ctx.buntu = false;
                    self.blok(b);
                    if !self.ctx.buntu {
                        self.emit(&format!("br label %{}", akhir));
                    }
                }
                if let Some(b) = lainnya {
                    self.ctx.badan.push_str(&format!("{}:\n", bawaan));
                    self.ctx.buntu = false;
                    self.blok(b);
                    if !self.ctx.buntu {
                        self.emit(&format!("br label %{}", akhir));
                    }
                }
                self.ctx.badan.push_str(&format!("{}:\n", akhir));
                self.ctx.buntu = false;
            }
            Stmt::Blok(b) => self.blok(b),
            Stmt::Takaman(b) => {
                self.takaman += 1;
                self.blok(b);
                self.takaman -= 1;
            }
        }
    }

    fn jaga_aktif(&self) -> bool {
        self.opsi.jaga && self.takaman == 0
    }

    fn jaga(&mut self, kondisi: &str, kode: i32, baris: u32) {
        if !self.jaga_aktif() {
            return;
        }
        let lo = self.lbl("jaga.ok");
        let lg = self.lbl("jaga.gagal");
        self.emit(&format!("br i1 {}, label %{}, label %{}", kondisi, lo, lg));
        self.ctx.badan.push_str(&format!("{}:\n", lg));
        self.ctx.buntu = false;
        self.emit(&format!("call void @nusa_gagal(i32 {}, i32 {})", kode, baris));
        self.emit("unreachable");
        self.ctx.badan.push_str(&format!("{}:\n", lo));
        self.ctx.buntu = false;
    }

    fn jaga_batas(&mut self, indeks: &str, panjang: &str, baris: u32) {
        let c = self.tmp();
        self.emit(&format!("{} = icmp ult i64 {}, {}", c, indeks, panjang));
        self.jaga(&c, 2, baris);
    }

    fn nilai_tanpa_pindah(&mut self, e: &Expr) -> Val {
        self.ctx.jangan_pindah += 1;
        let v = self.ekspresi(e);
        self.ctx.jangan_pindah -= 1;
        v
    }

    fn nilai_ref(&mut self, v: Val) -> Val {
        match v.ty {
            Ty::Ref { dalam, .. } => {
                let r = self.muat(&v.reg, &dalam);
                Val { reg: r, ty: *dalam }
            }
            _ => v,
        }
    }

    fn ptr_dan_panjang(&mut self, e: &Expr) -> (String, String) {
        let t = self.sema.tipe.get(&e.id).cloned().unwrap_or(Ty::Bil);
        let t = match t {
            Ty::Ref { dalam, .. } => *dalam,
            lain => lain,
        };
        match &t {
            Ty::Array(d, n) => {
                let v = self.nilai_tanpa_pindah(e);
                let v = self.nilai_ref(v);
                let et = self.llty(d);
                let at = self.llty(&t);
                let p = self.alloca(&at);
                self.simpan(&p, &t, &v.reg);
                let ep = self.tmp();
                self.emit(&format!("{} = getelementptr inbounds {}, {}* {}, i64 0, i64 0", ep, at, at, p, ));
                let _ = et;
                (ep, n.to_string())
            }
            Ty::Irisan(_) => {
                let v = self.nilai_tanpa_pindah(e);
                let v = self.nilai_ref(v);
                let it = self.llty(&t);
                let p = self.tmp();
                let l = self.tmp();
                self.emit(&format!("{} = extractvalue {} {}, 0", p, it, v.reg));
                self.emit(&format!("{} = extractvalue {} {}, 1", l, it, v.reg));
                (p, l)
            }
            Ty::Teks => {
                let v = self.nilai_tanpa_pindah(e);
                let v = self.nilai_ref(v);
                let p = self.tmp();
                let l = self.tmp();
                self.emit(&format!("{} = extractvalue %teks {}, 0", p, v.reg));
                self.emit(&format!("{} = extractvalue %teks {}, 1", l, v.reg));
                (p, l)
            }
            Ty::Daftar(d) => {
                let v = self.nilai_tanpa_pindah(e);
                let v = self.nilai_ref(v);
                let dt = self.llty(&Ty::Daftar(d.clone()));
                let p = self.tmp();
                let l = self.tmp();
                self.emit(&format!("{} = extractvalue {} {}, 0", p, dt, v.reg));
                self.emit(&format!("{} = extractvalue {} {}, 1", l, dt, v.reg));
                (p, l)
            }
            _ => {
                let v = self.nilai_tanpa_pindah(e);
                let et = self.llty(&t);
                let p = self.alloca(&et);
                self.simpan(&p, &t, &v.reg);
                (p, "1".to_string())
            }
        }
    }

    fn tempat_ptr(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Var(n) => match self.cari(n) {
                Some(l) => l.ptr,
                None => "%rusak".into(),
            },
            ExprKind::Medan { dasar, nama } => {
                let t = self.sema.tipe.get(&dasar.id).cloned().unwrap_or(Ty::Bil);
                let (bp, nama_struk) = match &t {
                    Ty::Ref { dalam, .. } => {
                        let v = self.ekspresi(dasar);
                        let n = match &**dalam {
                            Ty::Nama(n) => n.clone(),
                            _ => String::new(),
                        };
                        (v.reg, n)
                    }
                    Ty::Nama(n) => (self.tempat_ptr(dasar), n.clone()),
                    _ => (self.tempat_ptr(dasar), String::new()),
                };
                let idx = self
                    .sema
                    .struk
                    .get(&nama_struk)
                    .and_then(|s| s.indeks.get(nama).copied())
                    .unwrap_or(0);
                let st = format!("%struk.{}", nama_struk);
                let ty_medan = self
                    .sema
                    .struk
                    .get(&nama_struk)
                    .map(|s| s.medan[idx].1.clone())
                    .unwrap_or(Ty::Bil);
                let mt = self.llty(&ty_medan);
                let p = self.tmp();
                self.emit(&format!("{} = getelementptr {}, {}* {}, i32 0, i32 {}", p, st, st, bp, idx));
                let _ = mt;
                p
            }
            ExprKind::Indeks { dasar, indeks } => {
                let (base_ptr, len, elemen) = self.ptr_wadah(dasar);
                let ti = self.ekspresi(indeks);
                let idx64 = self.ke_i64(&ti);
                self.jaga_batas(&idx64, &len, e.span.baris);
                let et = self.llty(&elemen);
                let p = self.tmp();
                self.emit(&format!("{} = getelementptr inbounds {}, {}* {}, i64 {}", p, et, et, base_ptr, idx64));
                p
            }
            _ => {
                let v = self.ekspresi(e);
                let t = self.llty(&v.ty);
                let p = self.alloca(&t);
                self.simpan(&p, &v.ty, &v.reg);
                p
            }
        }
    }

    /// (pointer elemen, panjang, tipe elemen) untuk larik/irisan/daftar/teks.
    fn ptr_wadah(&mut self, e: &Expr) -> (String, String, Ty) {
        let t = self.sema.tipe.get(&e.id).cloned().unwrap_or(Ty::Bil);
        let t = match t {
            Ty::Ref { dalam, .. } => *dalam,
            lain => lain,
        };
        match &t {
            Ty::Array(d, n) => {
                let v = self.nilai_tanpa_pindah(e);
                let v = self.nilai_ref(v);
                let at = self.llty(&t);
                let p = self.alloca(&at);
                self.simpan(&p, &t, &v.reg);
                let ep = self.tmp();
                self.emit(&format!("{} = getelementptr inbounds {}, {}* {}, i64 0, i64 0", ep, at, at, p));
                (ep, n.to_string(), (**d).clone())
            }
            Ty::Irisan(d) | Ty::Daftar(d) => {
                let v = self.nilai_tanpa_pindah(e);
                let v = self.nilai_ref(v);
                let at = self.llty(&v.ty);
                let p = self.tmp();
                let l = self.tmp();
                self.emit(&format!("{} = extractvalue {} {}, 0", p, at, v.reg));
                self.emit(&format!("{} = extractvalue {} {}, 1", l, at, v.reg));
                (p, l, (**d).clone())
            }
            Ty::Teks => {
                let v = self.nilai_tanpa_pindah(e);
                let v = self.nilai_ref(v);
                let p = self.tmp();
                let l = self.tmp();
                self.emit(&format!("{} = extractvalue %teks {}, 0", p, v.reg));
                self.emit(&format!("{} = extractvalue %teks {}, 1", l, v.reg));
                (p, l, Ty::Huruf)
            }
            _ => {
                let v = self.nilai_tanpa_pindah(e);
                let et = self.llty(&t);
                let p = self.alloca(&et);
                self.simpan(&p, &t, &v.reg);
                (p, "1".to_string(), t.clone())
            }
        }
    }

    fn ke_i64(&mut self, v: &Val) -> String {
        let r = self.tmp();
        match v.ty {
            Ty::Bil | Ty::Nat => self.emit(&format!("{} = add i64 {}, 0", r, v.reg)),
            Ty::Bil32 => self.emit(&format!("{} = sext i32 {} to i64", r, v.reg)),
            Ty::Bil8 | Ty::Huruf => self.emit(&format!("{} = sext i8 {} to i64", r, v.reg)),
            Ty::Nat8 => self.emit(&format!("{} = zext i8 {} to i64", r, v.reg)),
            Ty::Log => self.emit(&format!("{} = zext i1 {} to i64", r, v.reg)),
            _ => self.emit(&format!("{} = add i64 0, 0", r)),
        }
        r
    }

    fn ekspresi_pindah(&mut self, e: &Expr) -> Val {
        self.ctx.jangan_pindah += 0;
        self.ekspresi(e)
    }

    fn ubah_tipe(&mut self, v: Val, t: &Ty) -> Val {
        if v.ty == *t {
            return v;
        }
        if let (Ty::Array(d, n), Ty::Irisan(td)) = (&v.ty, t) {
            if **d == **td {
                let at = self.llty(&v.ty);
                let et = self.llty(d);
                let p = self.alloca(&at);
                self.simpan(&p, &v.ty, &v.reg);
                let ep = self.tmp();
                self.emit(&format!("{} = getelementptr inbounds {}, {}* {}, i64 0, i64 0", ep, at, at, p));
                let it = self.llty(t);
                let r1 = self.tmp();
                let r2 = self.tmp();
                self.emit(&format!("{} = insertvalue {} undef, {}* {}, 0", r1, it, et, ep));
                self.emit(&format!("{} = insertvalue {} {}, i64 {}, 1", r2, it, r1, n));
                return Val { reg: r2, ty: t.clone() };
            }
        }
        v
    }

    fn ekspresi(&mut self, e: &Expr) -> Val {
        let t = self.sema.tipe.get(&e.id).cloned().unwrap_or(Ty::Bil);
        match &e.kind {
            ExprKind::Bil(v) => Val { reg: v.to_string(), ty: t },
            ExprKind::Huruf(v) => Val { reg: v.to_string(), ty: Ty::Huruf },
            ExprKind::Pecah(v) => {
                let mut s = format!("{:e}", v);
                if !s.contains('.') {
                    if let Some(i) = s.find('e') {
                        s.insert_str(i, ".0");
                    } else {
                        s.push_str(".0e0");
                    }
                }
                Val { reg: s, ty: Ty::Pecah }
            }
            ExprKind::Teks(s) => {
                let nama = self.teks_konst(s);
                let len = s.len();
                let r1 = self.tmp();
                let r2 = self.tmp();
                self.emit(&format!("{} = insertvalue %teks undef, i8* {}, 0", r1, nama));
                self.emit(&format!("{} = insertvalue %teks {}, i64 {}, 1", r2, r1, len));
                Val { reg: r2, ty: Ty::Teks }
            }
            ExprKind::Log(b) => Val { reg: b.to_string(), ty: Ty::Log },
            ExprKind::Var(n) => match self.cari(n) {
                Some(l) => {
                    let v = self.muat(&l.ptr, &l.ty);
                    if l.daftar && self.ctx.jangan_pindah == 0 {
                        self.simpan(&l.ptr, &l.ty, "zeroinitializer");
                    }
                    Val { reg: v, ty: l.ty }
                }
                None => Val { reg: "0".into(), ty: t },
            },
            ExprKind::Biner(op, a, b) => match op {
                BinOp::Dan | BinOp::Atau => self.dan_atau(*op, a, b),
                BinOp::Sama | BinOp::TidakSama | BinOp::Kecil | BinOp::KecilSama | BinOp::Besar | BinOp::BesarSama => self.bandingkan(*op, a, b, e.span.baris),
                _ => {
                    let x = self.ekspresi(a);
                    let y = self.ekspresi(b);
                    self.arits(*op, &x, &y, e.span.baris)
                }
            },
            ExprKind::Uner(op, a) => {
                let v = self.ekspresi(a);
                match op {
                    UnOp::Bukan => {
                        let r = self.tmp();
                        self.emit(&format!("{} = xor i1 {}, true", r, v.reg));
                        Val { reg: r, ty: Ty::Log }
                    }
                    UnOp::Neg => self.arits(BinOp::Kurang, &Val { reg: "0".into(), ty: v.ty.clone() }, &v, e.span.baris),
                }
            }
            ExprKind::Alamat { dasar, .. } => {
                let p = self.tempat_ptr(dasar);
                Val { reg: p, ty: t }
            }
            ExprKind::Indeks { dasar, indeks } => {
                let (base_ptr, len, elemen) = self.ptr_wadah(dasar);
                let ti = self.ekspresi(indeks);
                let idx = self.ke_i64(&ti);
                self.jaga_batas(&idx, &len, e.span.baris);
                let et = self.llty(&elemen);
                let p = self.tmp();
                self.emit(&format!("{} = getelementptr inbounds {}, {}* {}, i64 {}", p, et, et, base_ptr, idx));
                let v = self.muat(&p, &elemen);
                Val { reg: v, ty: elemen }
            }
            ExprKind::Medan { dasar, nama } => {
                let dt = self.sema.tipe.get(&dasar.id).cloned().unwrap_or(Ty::Bil);
                match dt {
                    Ty::Ref { dalam, .. } => {
                        let (nama_struk, ty_medan) = match &*dalam {
                            Ty::Nama(n) => {
                                let info = self.sema.struk.get(n).cloned();
                                let idx = info.as_ref().and_then(|s| s.indeks.get(nama).copied()).unwrap_or(0);
                                let tm = info.map(|s| s.medan[idx].1.clone()).unwrap_or(Ty::Bil);
                                (n.clone(), tm)
                            }
                            _ => (String::new(), Ty::Bil),
                        };
                        let _ = nama_struk;
                        let p = self.tempat_ptr(e);
                        let v = self.muat(&p, &ty_medan);
                        Val { reg: v, ty: ty_medan }
                    }
                    Ty::Nama(_) => {
                        let p = self.tempat_ptr(e);
                        let v = self.muat(&p, &t);
                        if self.ctx.jangan_pindah == 0 && self.sema.pindah.contains(&e.id) {
                            self.simpan(&p, &t, "zeroinitializer");
                        }
                        Val { reg: v, ty: t }
                    }
                    _ => Val { reg: "0".into(), ty: t },
                }
            }
            ExprKind::Sebagai { nilai, ty } => {
                let v = self.ekspresi(nilai);
                self.konversi(v, ty, e.span.baris)
            }
            ExprKind::StrukLit { nama, medan } => {
                let info = self.sema.struk.get(nama).cloned();
                let st = format!("%struk.{}", nama);
                let mut reg = "undef".to_string();
                if let Some(info) = info {
                    for (i, (mn, mty)) in info.medan.iter().enumerate() {
                        let nil = medan.iter().find(|(n, _)| n == mn).map(|(_, v)| v);
                        if let Some(v) = nil {
                            let mut vv = self.ekspresi(v);
                            vv = self.ubah_tipe(vv, mty);
                            let mt = self.llty(mty);
                            let r = self.tmp();
                            self.emit(&format!("{} = insertvalue {} {}, {} {}, {}", r, st, reg, mt, vv.reg, i));
                            reg = r;
                        }
                    }
                }
                Val { reg, ty: Ty::Nama(nama.clone()) }
            }
            ExprKind::Varian { enumerasi, varian } => {
                let idx = self
                    .sema
                    .enumerasi
                    .get(enumerasi)
                    .and_then(|e| e.varian.iter().position(|v| v == varian))
                    .unwrap_or(0);
                Val { reg: idx.to_string(), ty: Ty::Nama(enumerasi.clone()) }
            }
            ExprKind::ArrayLit(isi) => {
                let at = self.llty(&t);
                let et = match &t {
                    Ty::Array(d, _) => self.llty(d),
                    _ => "i64".into(),
                };
                let p = self.alloca(&at);
                for (i, x) in isi.iter().enumerate() {
                    let v = self.ekspresi(x);
                    let ep = self.tmp();
                    self.emit(&format!("{} = getelementptr inbounds {}, {}* {}, i64 0, i64 {}", ep, at, at, p, i));
                    self.emit(&format!("store {} {}, {}* {}", et, v.reg, et, ep));
                }
                let r = self.tmp();
                self.emit(&format!("{} = load {}, {}* {}", r, at, at, p));
                Val { reg: r, ty: t }
            }
            ExprKind::Panggil { nama, args } => self.panggilan(e, nama, args),
        }
    }

    fn dan_atau(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Val {
        let l = self.ekspresi(a);
        let p = self.alloca("i1");
        let awal = if op == BinOp::Dan { "false" } else { "true" };
        self.simpan(&p, &Ty::Log, awal);
        let lk = self.lbl("logika.kanan");
        let lt = self.lbl("logika.tamat");
        if op == BinOp::Dan {
            self.emit(&format!("br i1 {}, label %{}, label %{}", l.reg, lk, lt));
        } else {
            self.emit(&format!("br i1 {}, label %{}, label %{}", l.reg, lt, lk));
        }
        self.ctx.badan.push_str(&format!("{}:\n", lk));
        self.ctx.buntu = false;
        let r = self.ekspresi(b);
        self.simpan(&p, &Ty::Log, &r.reg);
        self.emit(&format!("br label %{}", lt));
        self.ctx.badan.push_str(&format!("{}:\n", lt));
        self.ctx.buntu = false;
        let v = self.muat(&p, &Ty::Log);
        Val { reg: v, ty: Ty::Log }
    }

    fn lebar(&self, t: &Ty) -> u32 {
        match t {
            Ty::Bil | Ty::Nat => 64,
            Ty::Bil32 => 32,
            Ty::Bil8 | Ty::Nat8 | Ty::Huruf => 8,
            Ty::Log => 1,
            _ => 64,
        }
    }

    fn bertanda(&self, t: &Ty) -> bool {
        matches!(t, Ty::Bil | Ty::Bil32 | Ty::Bil8)
    }

    fn arits(&mut self, op: BinOp, a: &Val, b: &Val, baris: u32) -> Val {
        let ty = a.ty.clone();
        if ty == Ty::Pecah {
            let ins = match op {
                BinOp::Tambah => "fadd",
                BinOp::Kurang => "fsub",
                BinOp::Kali => "fmul",
                BinOp::Bagi => "fdiv",
                BinOp::Sisa => "frem",
                _ => "fadd",
            };
            let r = self.tmp();
            self.emit(&format!("{} = {} double {}, {}", r, ins, a.reg, b.reg));
            return Val { reg: r, ty };
        }
        let w = self.lebar(&ty);
        let t = self.llty(&ty);
        let tanda = if self.bertanda(&ty) { "s" } else { "u" };
        match op {
            BinOp::Tambah | BinOp::Kurang | BinOp::Kali => {
                let nm = match op {
                    BinOp::Tambah => "add",
                    BinOp::Kurang => "sub",
                    _ => "mul",
                };
                if self.jaga_aktif() {
                    let intr = format!("@llvm.{}{}.with.overflow.i{}", tanda, nm, w);
                    let pr = self.tmp();
                    self.emit(&format!("{} = call {{ {}, i1 }} {}({} {}, {} {})", pr, t, intr, t, a.reg, t, b.reg));
                    let v = self.tmp();
                    let ov = self.tmp();
                    self.emit(&format!("{} = extractvalue {{ {}, i1 }} {}, 0", v, t, pr));
                    self.emit(&format!("{} = extractvalue {{ {}, i1 }} {}, 1", ov, t, pr));
                    let aman = self.tmp();
                    self.emit(&format!("{} = xor i1 {}, true", aman, ov));
                    self.jaga(&aman, 1, baris);
                    Val { reg: v, ty }
                } else {
                    let r = self.tmp();
                    self.emit(&format!("{} = {} n{}w {} {}, {}", r, nm, tanda, t, a.reg, b.reg));
                    Val { reg: r, ty }
                }
            }
            BinOp::Bagi | BinOp::Sisa => {
                let nol = self.tmp();
                self.emit(&format!("{} = icmp ne {} {}, 0", nol, t, b.reg));
                self.jaga(&nol, 3, baris);
                if self.bertanda(&ty) {
                    let mn = format!("{}", if w == 64 { i64::MIN } else if w == 32 { i32::MIN as i64 } else { i8::MIN as i64 });
                    let c1 = self.tmp();
                    let c2 = self.tmp();
                    let c3 = self.tmp();
                    self.emit(&format!("{} = icmp eq {} {}, {}", c1, t, a.reg, mn));
                    self.emit(&format!("{} = icmp eq {} {}, -1", c2, t, b.reg));
                    self.emit(&format!("{} = and i1 {}, {}", c3, c1, c2));
                    let aman = self.tmp();
                    self.emit(&format!("{} = xor i1 {}, true", aman, c3));
                    self.jaga(&aman, 1, baris);
                }
                let ins = if op == BinOp::Bagi {
                    if self.bertanda(&ty) { "sdiv" } else { "udiv" }
                } else if self.bertanda(&ty) {
                    "srem"
                } else {
                    "urem"
                };
                let r = self.tmp();
                self.emit(&format!("{} = {} {} {}, {}", r, ins, t, a.reg, b.reg));
                Val { reg: r, ty }
            }
            BinOp::GeserKiri | BinOp::GeserKanan => {
                let c = self.tmp();
                self.emit(&format!("{} = icmp ult {} {}, {}", c, t, b.reg, w));
                self.jaga(&c, 4, baris);
                let ins = match op {
                    BinOp::GeserKiri => "shl",
                    _ if self.bertanda(&ty) => "ashr",
                    _ => "lshr",
                };
                let r = self.tmp();
                self.emit(&format!("{} = {} {} {}, {}", r, ins, t, a.reg, b.reg));
                Val { reg: r, ty }
            }
            _ => Val { reg: "0".into(), ty },
        }
    }

    fn bandingkan(&mut self, op: BinOp, a: &Expr, b: &Expr, _baris: u32) -> Val {
        let x = self.ekspresi(a);
        let y = self.ekspresi(b);
        let ty = x.ty.clone();
        if ty == Ty::Teks {
            return self.banding_teks(op, &x, &y);
        }
        if ty == Ty::Pecah {
            let ins = match op {
                BinOp::Sama => "oeq",
                BinOp::TidakSama => "one",
                BinOp::Kecil => "olt",
                BinOp::KecilSama => "ole",
                BinOp::Besar => "ogt",
                BinOp::BesarSama => "oge",
                _ => "oeq",
            };
            let r = self.tmp();
            self.emit(&format!("{} = fcmp {} double {}, {}", r, ins, x.reg, y.reg));
            return Val { reg: r, ty: Ty::Log };
        }
        let nama_enum = matches!(&ty, Ty::Nama(n) if self.sema.enumerasi.contains_key(n));
        if matches!(ty, Ty::Nama(_)) && !nama_enum {
            let t = self.llty(&ty);
            let pa = self.alloca(&t);
            self.simpan(&pa, &ty, &x.reg);
            let pb = self.alloca(&t);
            self.simpan(&pb, &ty, &y.reg);
            let uk = self.ukuran(&ty);
            let r = self.tmp();
            let ba = self.tmp();
            let bb = self.tmp();
            self.emit(&format!("{} = bitcast {}* {} to i8*", ba, t, pa));
            self.emit(&format!("{} = bitcast {}* {} to i8*", bb, t, pb));
            self.emit(&format!("{} = call i32 @nusa_banding_memori(i8* {}, i8* {}, i64 {})", r, ba, bb, uk));
            let c = self.tmp();
            let ins = if op == BinOp::Sama { "eq" } else { "ne" };
            self.emit(&format!("{} = icmp {} i32 {}, 0", c, ins, r));
            return Val { reg: c, ty: Ty::Log };
        }
        let tanda = if self.bertanda(&ty) || matches!(ty, Ty::Nama(_)) { "s" } else { "u" };
        let t = if ty == Ty::Log { "i1".to_string() } else { self.llty(&ty) };
        let ins = match op {
            BinOp::Sama => "eq",
            BinOp::TidakSama => "ne",
            BinOp::Kecil => if tanda == "s" { "slt" } else { "ult" },
            BinOp::KecilSama => if tanda == "s" { "sle" } else { "ule" },
            BinOp::Besar => if tanda == "s" { "sgt" } else { "ugt" },
            BinOp::BesarSama => if tanda == "s" { "sge" } else { "uge" },
            _ => "eq",
        };
        let r = self.tmp();
        self.emit(&format!("{} = icmp {} {} {}, {}", r, ins, t, x.reg, y.reg));
        Val { reg: r, ty: Ty::Log }
    }

    fn banding_teks(&mut self, op: BinOp, x: &Val, y: &Val) -> Val {
        let px = self.tmp();
        let lx = self.tmp();
        let py = self.tmp();
        let ly = self.tmp();
        self.emit(&format!("{} = extractvalue %teks {}, 0", px, x.reg));
        self.emit(&format!("{} = extractvalue %teks {}, 1", lx, x.reg));
        self.emit(&format!("{} = extractvalue %teks {}, 0", py, y.reg));
        self.emit(&format!("{} = extractvalue %teks {}, 1", ly, y.reg));
        if op == BinOp::Sama || op == BinOp::TidakSama {
            let r = self.tmp();
            self.emit(&format!("{} = call i1 @nusa_teks_sama(i8* {}, i64 {}, i8* {}, i64 {})", r, px, lx, py, ly));
            if op == BinOp::TidakSama {
                let n = self.tmp();
                self.emit(&format!("{} = xor i1 {}, true", n, r));
                return Val { reg: n, ty: Ty::Log };
            }
            return Val { reg: r, ty: Ty::Log };
        }
        let r = self.tmp();
        self.emit(&format!("{} = call i32 @nusa_teks_banding(i8* {}, i64 {}, i8* {}, i64 {})", r, px, lx, py, ly));
        let c = self.tmp();
        let ins = match op {
            BinOp::Kecil => "slt",
            BinOp::KecilSama => "sle",
            BinOp::Besar => "sgt",
            BinOp::BesarSama => "sge",
            _ => "eq",
        };
        self.emit(&format!("{} = icmp {} i32 {}, 0", c, ins, r));
        Val { reg: c, ty: Ty::Log }
    }

    fn konversi(&mut self, v: Val, ty: &Ty, baris: u32) -> Val {
        let asal = v.ty.clone();
        if asal == *ty {
            return v;
        }
        if asal == Ty::Pecah || *ty == Ty::Pecah {
            let r = self.tmp();
            if asal == Ty::Pecah {
                let t = self.llty(ty);
                let tanda = if self.bertanda(ty) { "fptosi" } else { "fptoui" };
                if self.jaga_aktif() {
                    let (lo, atas) = match ty {
                        Ty::Bil => (i64::MIN as f64, 9223372036854775808.0),
                        Ty::Bil32 => (i32::MIN as f64, 2147483648.0),
                        Ty::Bil8 => (-128.0, 128.0),
                        Ty::Nat => (0.0, 18446744073709551616.0),
                        Ty::Nat8 | Ty::Huruf => (0.0, 256.0),
                        Ty::Log => (0.0, 2.0),
                        _ => (i64::MIN as f64, 9223372036854775808.0),
                    };
                    let ord = self.tmp();
                    let bawah = self.tmp();
                    let atasr = self.tmp();
                    let a = self.tmp();
                    let b = self.tmp();
                    self.emit(&format!("{} = fcmp ord double {}, {}", ord, v.reg, v.reg));
                    self.emit(&format!("{} = fcmp oge double {}, {:.1}", bawah, v.reg, lo));
                    self.emit(&format!("{} = fcmp olt double {}, {:.1}", atasr, v.reg, atas));
                    self.emit(&format!("{} = and i1 {}, {}", a, ord, bawah));
                    self.emit(&format!("{} = and i1 {}, {}", b, a, atasr));
                    self.jaga(&b, 1, baris);
                }
                self.emit(&format!("{} = {} double {} to {}", r, tanda, v.reg, t));
            } else {
                let t = self.llty(&asal);
                let tanda = if self.bertanda(&asal) { "sitofp" } else { "uitofp" };
                self.emit(&format!("{} = {} {} {} to double", r, tanda, t, v.reg));
            }
            return Val { reg: r, ty: ty.clone() };
        }
        let wa = self.lebar(&asal);
        let wt = self.lebar(ty);
        let ta = self.llty(&asal);
        let tt = self.llty(ty);
        let r = self.tmp();
        if wt > wa {
            let op = if self.bertanda(&asal) { "sext" } else { "zext" };
            self.emit(&format!("{} = {} {} {} to {}", r, op, ta, v.reg, tt));
        } else if wt < wa {
            if self.jaga_aktif() {
                let extended = if wa >= 64 {
                    v.reg.clone()
                } else {
                    let t = self.tmp();
                    let op = if self.bertanda(&asal) { "sext" } else { "zext" };
                    self.emit(&format!("{} = {} {} {} to i64", t, op, ta, v.reg));
                    t
                };
                let (lo, hi) = batas(ty);
                let c3 = self.tmp();
                if wa >= 64 && !self.bertanda(&asal) {
                    self.emit(&format!("{} = icmp ule i64 {}, {}", c3, extended, hi));
                } else {
                    let c1 = self.tmp();
                    let c2 = self.tmp();
                    self.emit(&format!("{} = icmp sge i64 {}, {}", c1, extended, lo));
                    self.emit(&format!("{} = icmp sle i64 {}, {}", c2, extended, hi));
                    self.emit(&format!("{} = and i1 {}, {}", c3, c1, c2));
                }
                self.jaga(&c3, 1, baris);
            }
            self.emit(&format!("{} = trunc {} {} to {}", r, ta, v.reg, tt));
        } else {
            self.emit(&format!("{} = bitcast {} {} to {}", r, ta, v.reg, tt));
        }
        Val { reg: r, ty: ty.clone() }
    }

    fn panggilan(&mut self, e: &Expr, _nama: &str, args: &[Expr]) -> Val {
        let jenis = self.sema.panggil.get(&e.id).cloned();
        match jenis {
            Some(Panggilan::Fungsi(n)) => {
                let sig = self.sema.sig.get(&n).cloned();
                let (params, hasil) = match sig {
                    Some(s) => (s.params, s.hasil),
                    None => (vec![], Ty::Kosong),
                };
                let mut argv = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let v = self.ekspresi(a);
                    let harap = params.get(i).cloned().unwrap_or(v.ty.clone());
                    let v = self.ubah_tipe(v, &harap);
                    let t = self.llty(&v.ty);
                    argv.push(format!("{} {}", t, v.reg));
                }
                let ret = self.llty(&hasil);
                let r = self.tmp();
                if hasil == Ty::Kosong {
                    self.emit(&format!("call void @nusa_{}({})", n, argv.join(", ")));
                    let _ = r;
                    Val { reg: "0".into(), ty: Ty::Kosong }
                } else {
                    self.emit(&format!("{} = call {} @nusa_{}({})", r, ret, n, argv.join(", ")));
                    Val { reg: r, ty: hasil }
                }
            }
            Some(Panggilan::Bawaan(n)) => self.bawaan(e, &n, args),
            None => Val { reg: "0".into(), ty: Ty::Kosong },
        }
    }

    fn cetak_satu(&mut self, etiket: &str, v: &Val) {
        match &v.ty {
            Ty::Bil => self.emit(&format!("call void @nusa_{}_bil(i64 {})", etiket, v.reg)),
            Ty::Nat => self.emit(&format!("call void @nusa_{}_nat(i64 {})", etiket, v.reg)),
            Ty::Bil32 => {
                let r = self.tmp();
                self.emit(&format!("{} = sext i32 {} to i64", r, v.reg));
                self.emit(&format!("call void @nusa_{}_bil(i64 {})", etiket, r));
            }
            Ty::Bil8 => {
                let r = self.tmp();
                self.emit(&format!("{} = sext i8 {} to i64", r, v.reg));
                self.emit(&format!("call void @nusa_{}_bil(i64 {})", etiket, r));
            }
            Ty::Nat8 => {
                let r = self.tmp();
                self.emit(&format!("{} = zext i8 {} to i64", r, v.reg));
                self.emit(&format!("call void @nusa_{}_nat(i64 {})", etiket, r));
            }
            Ty::Huruf => self.emit(&format!("call void @nusa_{}_huruf(i8 {})", etiket, v.reg)),
            Ty::Pecah => self.emit(&format!("call void @nusa_{}_pecah(double {})", etiket, v.reg)),
            Ty::Log => {
                let r = self.tmp();
                self.emit(&format!("{} = zext i1 {} to i32", r, v.reg));
                self.emit(&format!("call void @nusa_{}_log(i32 {})", etiket, r));
            }
            Ty::Teks => {
                let p = self.tmp();
                let l = self.tmp();
                self.emit(&format!("{} = extractvalue %teks {}, 0", p, v.reg));
                self.emit(&format!("{} = extractvalue %teks {}, 1", l, v.reg));
                self.emit(&format!("call void @nusa_{}_teks(i8* {}, i64 {})", etiket, p, l));
            }
            Ty::Nama(n) => {
                let tabel = match self.varian.get(n) {
                    Some(t) => t.clone(),
                    None => String::new(),
                };
                if tabel.is_empty() {
                    let r = self.tmp();
                    self.emit(&format!("{} = sext i32 {} to i64", r, v.reg));
                    self.emit(&format!("call void @nusa_{}_bil(i64 {})", etiket, r));
                } else {
                    let jml = self.sema.enumerasi.get(n).map(|e| e.varian.len()).unwrap_or(0);
                    let p = self.tmp();
                    self.emit(&format!("{} = getelementptr inbounds [{} x %teks], [{} x %teks]* {}, i64 0, i64 0", p, jml, jml, tabel, ));
                    self.emit(&format!("call void @nusa_{}_varian(i32 {}, %teks* {}, i32 {})", etiket, v.reg, p, jml));
                }
            }
            _ => {}
        }
    }

    fn bawaan(&mut self, e: &Expr, nama: &str, args: &[Expr]) -> Val {
        match nama {
            "cetak" | "tulis" => {
                for a in args {
                    let v = self.ekspresi(a);
                    self.cetak_satu("tulis", &v);
                }
                if nama == "cetak" {
                    self.emit("call void @nusa_baris()");
                }
                Val { reg: "0".into(), ty: Ty::Kosong }
            }
            "daftar" => {
                let t = self.sema.tipe.get(&e.id).cloned().unwrap_or(Ty::Daftar(Box::new(Ty::Bil)));
                let dt = self.llty(&t);
                let r1 = self.tmp();
                let r2 = self.tmp();
                let r3 = self.tmp();
                self.emit(&format!("{} = insertvalue {} undef, i8* null, 0", r1, dt));
                self.emit(&format!("{} = insertvalue {} {}, i64 0, 1", r2, dt, r1));
                self.emit(&format!("{} = insertvalue {} {}, i64 0, 2", r3, dt, r2));
                Val { reg: r3, ty: t }
            }
            "tambah" | "sisip" => {
                let t0 = self.sema.tipe.get(&args[0].id).cloned().unwrap_or(Ty::Bil);
                let elemen = match &t0 {
                    Ty::Ref { dalam, .. } => match &**dalam {
                        Ty::Daftar(t) => (**t).clone(),
                        _ => Ty::Bil,
                    },
                    Ty::Daftar(t) => (**t).clone(),
                    _ => Ty::Bil,
                };
                let d = if self.sema.metode.contains(&e.id) {
                    Val { reg: self.tempat_ptr(&args[0]), ty: t0.clone() }
                } else {
                    self.ekspresi(&args[0])
                };
                let dt = self.llty(&Ty::Daftar(Box::new(elemen.clone())));
                let dp = self.tmp();
                let lp = self.tmp();
                let cp = self.tmp();
                self.emit(&format!("{} = getelementptr {}, {}* {}, i32 0, i32 0", dp, dt, dt, d.reg));
                self.emit(&format!("{} = getelementptr {}, {}* {}, i32 0, i32 1", lp, dt, dt, d.reg));
                self.emit(&format!("{} = getelementptr {}, {}* {}, i32 0, i32 2", cp, dt, dt, d.reg));
                let idx = if nama == "sisip" { Some(self.ekspresi(&args[1])) } else { None };
                let nilai_idx = if nama == "sisip" { 2 } else { 1 };
                let v = self.ekspresi(&args[nilai_idx]);
                let v = self.ubah_tipe(v, &elemen);
                let et = self.llty(&elemen);
                let tmp = self.alloca(&et);
                self.simpan(&tmp, &elemen, &v.reg);
                let uk = self.ukuran(&elemen);
                let bc = self.tmp();
                self.emit(&format!("{} = bitcast {}* {} to i8*", bc, et, tmp));
                if let Some(iv) = idx {
                    let idx64 = self.ke_i64(&iv);
                    let lv = self.muat(&lp, &Ty::Bil);
                    let c = self.tmp();
                    self.emit(&format!("{} = icmp ule i64 {}, {}", c, idx64, lv));
                    self.jaga(&c, 2, e.span.baris);
                    self.emit(&format!(
                        "call void @nusa_daftar_sisip(i8** {}, i64* {}, i64* {}, i64 {}, i8* {}, i64 {})",
                        dp, lp, cp, idx64, bc, uk
                    ));
                } else {
                    self.emit(&format!(
                        "call void @nusa_daftar_tambah(i8** {}, i64* {}, i64* {}, i8* {}, i64 {})",
                        dp, lp, cp, bc, uk
                    ));
                }
                Val { reg: "0".into(), ty: Ty::Kosong }
            }
            "panjang" => {
                let a = &args[0];
                let t = self.sema.tipe.get(&a.id).cloned().unwrap_or(Ty::Bil);
                let t = match t {
                    Ty::Ref { dalam, .. } => *dalam,
                    lain => lain,
                };
                if let Ty::Array(_, n) = t {
                    return Val { reg: n.to_string(), ty: Ty::Bil };
                }
                let v = self.nilai_tanpa_pindah(a);
                let v = self.nilai_ref(v);
                let at = self.llty(&v.ty);
                let r = self.tmp();
                self.emit(&format!("{} = extractvalue {} {}, 1", r, at, v.reg));
                Val { reg: r, ty: Ty::Bil }
            }
            "ambil" => {
                let (base_ptr, len, elemen) = self.ptr_wadah(&args[0]);
                let idx = self.ekspresi(&args[1]);
                let i64i = self.ke_i64(&idx);
                self.jaga_batas(&i64i, &len, e.span.baris);
                let et = self.llty(&elemen);
                let p = self.tmp();
                self.emit(&format!("{} = getelementptr inbounds {}, {}* {}, i64 {}", p, et, et, base_ptr, i64i));
                let v = self.muat(&p, &elemen);
                Val { reg: v, ty: elemen }
            }
            "gagal" => {
                let v = self.ekspresi(&args[0]);
                let p = self.tmp();
                let l = self.tmp();
                self.emit(&format!("{} = extractvalue %teks {}, 0", p, v.reg));
                self.emit(&format!("{} = extractvalue %teks {}, 1", l, v.reg));
                self.emit(&format!("call void @nusa_panic(i8* {}, i64 {})", p, l));
                self.emit("unreachable");
                self.ctx.buntu = true;
                Val { reg: "0".into(), ty: Ty::Kosong }
            }
            _ => Val { reg: "0".into(), ty: Ty::Kosong },
        }
    }

    fn tutup_blok(&mut self) {
        let perlu = match self.ctx.badan.lines().rev().find(|l| !l.trim().is_empty()) {
            Some(l) => {
                let l = l.trim_start();
                !(l.starts_with("ret ") || l.starts_with("br ") || l == "unreachable" || l.ends_with(':'))
            }
            None => false,
        };
        if perlu {
            self.emit("unreachable");
        }
    }

    fn fungsi(&mut self, f: &Fungsi) {
        self.tmp = 0;
        self.label = 0;
        self.ctx = FnCtx::kosong();
        self.ctx.hasil = f.hasil.clone();
        self.ctx.nama = f.nama.clone();
        let mut sig = Vec::new();
        for p in &f.params {
            let t = self.llty(&p.ty);
            let pn = format!("%p.{}", p.nama);
            sig.push(format!("{} {}", t, pn));
            let a = self.alloca(&t);
            self.simpan(&a, &p.ty, &pn);
            let daftar = self.perlu_bebas(&p.ty);
            self.ikat(&p.nama, Lokal { ptr: a, ty: p.ty.clone(), daftar });
        }
        for s in &f.badan.stmts {
            self.stmt(s);
        }
        if !self.ctx.buntu {
            if f.hasil == Ty::Kosong {
                self.emit("ret void");
            } else {
                self.emit("unreachable");
            }
        }
        self.keluar();
        self.tutup_blok();
        let ret = self.llty(&f.hasil);
        let teks = format!(
            "define {} @nusa_{}({}) {{\n{}{}}}\n",
            ret,
            f.nama,
            sig.join(", "),
            self.ctx.alloca,
            self.ctx.badan
        );
        self.fns.push(teks);
    }

    fn utama(&mut self) -> String {
        let hasil = self.sema.sig.get("utama").map(|s| s.hasil.clone()).unwrap_or(Ty::Kosong);
        if hasil == Ty::Kosong {
            "define i32 @main() {\n  call void @nusa_utama()\n  ret i32 0\n}\n".to_string()
        } else {
            let r = self.tmp();
            format!(
                "define i32 @main() {{\n  {} = call i64 @nusa_utama()\n  %kode = trunc i64 {} to i32\n  ret i32 %kode\n}}\n",
                r, r
            )
        }
    }

    pub fn hasil(mut self) -> String {
        let prog_fns: Vec<Fungsi> = self.prog.fungsi.clone();
        for f in &prog_fns {
            if !f.luar {
                self.fungsi(f);
            }
        }
        let utama = self.utama();
        let mut m = String::new();
        m.push_str(&format!("; Modul Nusa (opt={})\n", self.opsi.opt));
        m.push_str("target triple = \"x86_64-pc-linux-gnu\"\n\n");
        m.push_str("%teks = type { i8*, i64 }\n");
        for (nama, info) in self.sema.struk.clone() {
            let mut medan = Vec::new();
            for (_, t) in &info.medan {
                medan.push(self.llty(t));
            }
            m.push_str(&format!("%struk.{} = type {{ {} }}\n", nama, medan.join(", ")));
        }
        let daftar: Vec<(String, Ty)> = self.daftar.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (k, t) in daftar {
            let e = self.llty(&t);
            m.push_str(&format!("%daftar.{} = type {{ {}*, i64, i64 }}\n", k, e));
        }
        let irisan: Vec<(String, Ty)> = self.irisan.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (k, t) in irisan {
            let e = self.llty(&t);
            m.push_str(&format!("%irisan.{} = type {{ {}*, i64 }}\n", k, e));
        }
        let enum_urut: Vec<(String, String, Vec<String>)> = self
            .sema
            .enumerasi
            .iter()
            .map(|(k, v)| (k.clone(), self.varian.get(k).cloned().unwrap_or_default(), v.varian.clone()))
            .collect();
        for (_nama_enum, label, varian) in enum_urut {
            let mut isi = Vec::new();
            for v in &varian {
                let l = self.teks_konst(v);
                isi.push(format!("%teks {{ i8* {}, i64 {} }}", l, v.len()));
            }
            m.push_str(&format!("{} = private unnamed_addr constant [{} x %teks] [{}]\n", label, varian.len(), isi.join(", ")));
        }
        m.push('\n');
        for (nama, isi) in &self.teks {
            let mut s = String::new();
            for b in isi {
                match *b {
                    0x20..=0x21 | 0x23..=0x5B | 0x5D..=0x7E => s.push(*b as char),
                    lain => s.push_str(&format!("\\{:02X}", lain)),
                }
            }
            m.push_str(&format!("{} = private unnamed_addr constant [{} x i8] c\"{}\"\n", nama, isi.len(), s));
        }
        m.push('\n');
        m.push_str(&self.ekstern());
        m.push('\n');
        for f in &self.fns {
            m.push_str(f);
            m.push('\n');
        }
        m.push_str(&utama);
        m
    }

    fn ekstern(&self) -> String {
        let mut s = String::new();
        for w in [8u32, 32, 64] {
            for (tanda, op) in [("s", "add"), ("s", "sub"), ("s", "mul"), ("u", "add"), ("u", "sub"), ("u", "mul")] {
                s.push_str(&format!(
                    "declare {{ i{}, i1 }} @llvm.{}{}.with.overflow.i{}(i{}, i{})\n",
                    w, tanda, op, w, w, w
                ));
            }
        }
        s.push_str("declare void @nusa_gagal(i32, i32)\n");
        s.push_str("declare void @nusa_tegas(i8*, i64, i32)\n");
        s.push_str("declare void @nusa_panic(i8*, i64)\n");
        s.push_str("declare void @nusa_baris()\n");
        s.push_str("declare void @nusa_daftar_tambah(i8**, i64*, i64*, i8*, i64)\n");
        s.push_str("declare void @nusa_daftar_sisip(i8**, i64*, i64*, i64, i8*, i64)\n");
        s.push_str("declare void @nusa_daftar_bebas(i8*)\n");
        s.push_str("declare i1 @nusa_teks_sama(i8*, i64, i8*, i64)\n");
        s.push_str("declare i32 @nusa_teks_banding(i8*, i64, i8*, i64)\n");
        s.push_str("declare i32 @nusa_banding_memori(i8*, i8*, i64)\n");
        for e in ["cetak", "tulis"] {
            s.push_str(&format!("declare void @nusa_{}_bil(i64)\n", e));
            s.push_str(&format!("declare void @nusa_{}_nat(i64)\n", e));
            s.push_str(&format!("declare void @nusa_{}_pecah(double)\n", e));
            s.push_str(&format!("declare void @nusa_{}_log(i32)\n", e));
            s.push_str(&format!("declare void @nusa_{}_teks(i8*, i64)\n", e));
            s.push_str(&format!("declare void @nusa_{}_huruf(i8)\n", e));
            s.push_str(&format!("declare void @nusa_{}_varian(i32, %teks*, i32)\n", e));
        }
        s
    }
}

fn batas(t: &Ty) -> (i64, i64) {
    match t {
        Ty::Bil => (i64::MIN, i64::MAX),
        Ty::Bil32 => (i32::MIN as i64, i32::MAX as i64),
        Ty::Bil8 => (i8::MIN as i64, i8::MAX as i64),
        Ty::Nat => (0, i64::MAX),
        Ty::Nat8 => (0, u8::MAX as i64),
        Ty::Huruf => (0, u8::MAX as i64),
        Ty::Log => (0, 1),
        _ => (i64::MIN, i64::MAX),
    }
}
