//! Pengurai (parser) bahasa NUSA.

use crate::ast::*;
use crate::diag::Diagnostik;
use crate::lex::Tok;

pub struct Pengurai<'a> {
    tok: &'a [(Tok, Span)],
    pos: usize,
    berkas: &'a str,
    diag: Vec<Diagnostik>,
    id: u32,
    larang_struk_lit: u32,
}

pub struct HasilUrai {
    pub prog: Prog,
    pub diag: Vec<Diagnostik>,
}

pub fn urai(tok: &[(Tok, Span)], berkas: &str) -> HasilUrai {
    let mut p = Pengurai {
        tok,
        pos: 0,
        berkas,
        diag: Vec::new(),
        id: 1,
        larang_struk_lit: 0,
    };
    let prog = p.program();
    HasilUrai { prog, diag: p.diag }
}

impl<'a> Pengurai<'a> {
    fn id_baru(&mut self) -> u32 {
        let i = self.id;
        self.id += 1;
        i
    }

    fn intip(&self) -> &Tok {
        self.cap();
        self.tok.get(self.pos).map(|t| &t.0).unwrap_or(&Tok::Selesai)
    }

    fn cap(&self) {
        if self.pos > self.tok.len() * 64 + 10000 {
            panic!("pengurai berputar tanpa kemajuan pada posisi {} ({:?})", self.pos, self.intip());
        }
    }

    fn intip_span(&self) -> Span {
        self.tok.get(self.pos).map(|t| t.1).unwrap_or_default()
    }

    #[allow(dead_code)]
    fn intip_n(&self, n: usize) -> &Tok {
        self.tok.get(self.pos + n).map(|t| &t.0).unwrap_or(&Tok::Selesai)
    }

    fn maju(&mut self) -> (Tok, Span) {
        self.cap();
        let t = self
            .tok
            .get(self.pos)
            .cloned()
            .unwrap_or((Tok::Selesai, Span::default()));
        if self.pos < self.tok.len() {
            self.pos += 1;
        }
        t
    }

    fn makan(&mut self, t: &Tok) -> bool {
        if self.intip() == t {
            self.maju();
            true
        } else {
            false
        }
    }

    fn harap(&mut self, t: Tok, konteks: &str) -> Option<Span> {
        if self.intip() == &t {
            Some(self.maju().1)
        } else {
            let sp = self.intip_span();
            self.diag.push(Diagnostik::galat(
                self.berkas,
                sp,
                format!("mengharapkan {} pada {}, tetapi menemukan {}", t.sebutan(), konteks, self.intip().sebutan()),
                "E0101",
            ));
            None
        }
    }

    fn harap_ident(&mut self, konteks: &str) -> Option<(String, Span)> {
        match self.intip().clone() {
            Tok::Ident(n) => {
                let sp = self.maju().1;
                Some((n, sp))
            }
            lain => {
                let sp = self.intip_span();
                self.diag.push(Diagnostik::galat(
                    self.berkas,
                    sp,
                    format!("mengharapkan nama pada {}, tetapi menemukan {}", konteks, lain.sebutan()),
                    "E0102",
                ));
                None
            }
        }
    }

    fn lompat_ke_batas(&mut self) {
        loop {
            match self.intip() {
                Tok::TitikKoma => {
                    self.maju();
                    return;
                }
                Tok::RBrace | Tok::Selesai => return,
                Tok::Fungsi | Tok::Struk | Tok::Enumerasi => return,
                _ => {
                    self.maju();
                }
            }
        }
    }

    fn program(&mut self) -> Prog {
        let mut prog = Prog::default();
        loop {
            match self.intip() {
                Tok::Selesai => break,
                Tok::Impor => {
                    self.maju();
                    if let Tok::Teks(nama) = self.intip().clone() {
                        self.maju();
                        prog.impor.push(nama);
                    } else {
                        let sp = self.intip_span();
                        self.diag.push(Diagnostik::galat(
                            self.berkas,
                            sp,
                            "`impor` memerlukan teks nama modul, mis. `impor \"matematika\"`".into(),
                            "E0103",
                        ));
                        self.lompat_ke_batas();
                    }
                    self.makan(&Tok::TitikKoma);
                }
                Tok::Struk => {
                    if let Some(s) = self.struk() {
                        prog.struk.push(s);
                    }
                }
                Tok::Enumerasi => {
                    if let Some(e) = self.enumerasi() {
                        prog.enumerasi.push(e);
                    }
                }
                Tok::Luar | Tok::Fungsi => {
                    if let Some(f) = self.fungsi() {
                        prog.fungsi.push(f);
                    }
                }
                _ => {
                    let sp = self.intip_span();
                    self.diag.push(Diagnostik::galat(
                        self.berkas,
                        sp,
                        format!(
                            "hanya `fungsi`, `struk`, `enumerasi`, atau `impor` yang boleh di tingkat teratas, bukan {}",
                            self.intip().sebutan()
                        ),
                        "E0104",
                    ));
                    let sebelum = self.pos;
                    self.lompat_ke_batas();
                    if self.pos == sebelum {
                        self.maju();
                    }
                    if self.intip() == &Tok::Selesai {
                        break;
                    }
                }
            }
        }
        prog
    }

    fn struk(&mut self) -> Option<Struk> {
        let mulai = self.maju().1;
        let (nama, _) = self.harap_ident("`struk`")?;
        self.harap(Tok::LBrace, "badan struk")?;
        let mut medan = Vec::new();
        loop {
            if self.makan(&Tok::RBrace) {
                break;
            }
            if self.intip() == &Tok::Selesai {
                self.diag.push(Diagnostik::galat(
                    self.berkas,
                    mulai,
                    format!("struk `{}` tidak ditutup dengan `}}`", nama),
                    "E0105",
                ));
                return None;
            }
            let Some((mn, msp)) = self.harap_ident("nama medan") else {
                self.lompat_ke_batas();
                continue;
            };
            self.harap(Tok::TitikDua, "medan struk");
            let ty = self.tipe();
            medan.push(Param { nama: mn, ty, span: msp });
            if !self.makan(&Tok::Koma) {
                self.harap(Tok::RBrace, "akhir daftar medan")?;
                break;
            }
        }
        self.makan(&Tok::TitikKoma);
        Some(Struk { nama, medan, span: mulai })
    }

    fn enumerasi(&mut self) -> Option<Enumerasi> {
        let mulai = self.maju().1;
        let (nama, _) = self.harap_ident("`enumerasi`")?;
        self.harap(Tok::LBrace, "badan enumerasi")?;
        let mut varian = Vec::new();
        loop {
            if self.makan(&Tok::RBrace) {
                break;
            }
            if self.intip() == &Tok::Selesai {
                return None;
            }
            match self.harap_ident("nama varian") {
                Some((v, _)) => varian.push(v),
                None => self.lompat_ke_batas(),
            }
            if !self.makan(&Tok::Koma) {
                self.harap(Tok::RBrace, "akhir daftar varian")?;
                break;
            }
        }
        self.makan(&Tok::TitikKoma);
        Some(Enumerasi { nama, varian, span: mulai })
    }

    fn fungsi(&mut self) -> Option<Fungsi> {
        let mulai = self.intip_span();
        let luar = self.makan(&Tok::Luar);
        self.harap(Tok::Fungsi, "awal fungsi")?;
        let (nama, _) = self.harap_ident("nama fungsi")?;
        self.harap(Tok::LPar, "daftar parameter")?;
        let mut params = Vec::new();
        if !self.makan(&Tok::RPar) {
            loop {
                let Some((pn, psp)) = self.harap_ident("nama parameter") else {
                    self.lompat_ke_batas();
                    return None;
                };
                let ty = if self.makan(&Tok::TitikDua) {
                    self.tipe()
                } else {
                    let sp = self.intip_span();
                    self.diag.push(
                        Diagnostik::galat(
                            self.berkas,
                            sp,
                            format!("parameter `{}` memerlukan anotasi tipe, mis. `{}: bil`", pn, pn),
                            "E0106",
                        )
                        .saran("NUSA selalu menuntut tipe pada batas fungsi agar aman tipe."),
                    );
                    Ty::Bil
                };
                params.push(Param { nama: pn, ty, span: psp });
                if !self.makan(&Tok::Koma) {
                    break;
                }
            }
            self.harap(Tok::RPar, "akhir parameter")?;
        }
        let hasil = if self.makan(&Tok::Panah) { self.tipe() } else { Ty::Kosong };
        let mut perlu = Vec::new();
        let mut jamin = Vec::new();
        loop {
            match self.intip() {
                Tok::Perlu => {
                    self.maju();
                    let e = self.kondisi();
                    perlu.push(e);
                }
                Tok::Jamin => {
                    self.maju();
                    let e = self.kondisi();
                    jamin.push(e);
                }
                _ => break,
            }
        }
        let badan = if luar {
            if let Some(sp) = self.harap(Tok::TitikKoma, "akhir deklarasi `luar fungsi`") {
                Block { stmts: Vec::new(), span: sp }
            } else {
                Block { stmts: Vec::new(), span: mulai }
            }
        } else {
            self.blok()
        };
        Some(Fungsi { nama, params, hasil, perlu, jamin, badan, span: mulai, luar })
    }

    fn blok(&mut self) -> Block {
        let sp = self.intip_span();
        if self.harap(Tok::LBrace, "awal blok").is_none() {
            return Block { stmts: Vec::new(), span: sp };
        }
        let mut stmts = Vec::new();
        loop {
            if self.makan(&Tok::RBrace) {
                break;
            }
            if self.intip() == &Tok::Selesai {
                self.diag.push(Diagnostik::galat(
                    self.berkas,
                    sp,
                    "blok tidak ditutup dengan `}`".into(),
                    "E0107",
                ));
                break;
            }
            let sebelum = self.pos;
            if let Some(s) = self.stmt() {
                stmts.push(s);
            }
            if self.pos == sebelum {
                self.maju();
            }
        }
        Block { stmts, span: sp }
    }

    fn stmt(&mut self) -> Option<Stmt> {
        let sp = self.intip_span();
        match self.intip().clone() {
            Tok::Tetap | Tok::Ubah => {
                let tetap = self.intip() == &Tok::Tetap;
                self.maju();
                let (nama, nsp) = self.harap_ident("nama variabel")?;
                let ty = if self.makan(&Tok::TitikDua) { Some(self.tipe()) } else { None };
                let nilai = if self.makan(&Tok::Sama) {
                    self.ekspresi()
                } else {
                    self.diag.push(Diagnostik::galat(
                        self.berkas,
                        nsp,
                        format!("variabel `{}` harus langsung diberi nilai awal", nama),
                        "E0108",
                    ));
                    self.eks_bil(0, nsp)
                };
                self.makan(&Tok::TitikKoma);
                Some(Stmt::Ikat { tetap, nama, ty, nilai, span: sp })
            }
            Tok::Jika => {
                self.maju();
                let kond = self.kondisi();
                let maka = self.blok();
                let lain = if self.makan(&Tok::Selain) {
                    if self.intip() == &Tok::Jika {
                        let sp2 = self.intip_span();
                        let dalam = self.stmt()?;
                        Some(Block { stmts: vec![dalam], span: sp2 })
                    } else {
                        Some(self.blok())
                    }
                } else {
                    None
                };
                Some(Stmt::Jika { kond, maka, lain, span: sp })
            }
            Tok::Selama => {
                self.maju();
                let kond = self.kondisi();
                let mut invarian = Vec::new();
                while self.makan(&Tok::Invarian) {
                    invarian.push(self.kondisi());
                }
                let badan = self.blok();
                Some(Stmt::Selama { kond, invarian, badan, span: sp })
            }
            Tok::Untuk => {
                self.maju();
                let (nama, _) = self.harap_ident("variabel `untuk`")?;
                self.harap(Tok::Dalam, "`untuk`")?;
                self.larang_struk_lit += 1;
                let awal = self.ekspresi();
                let rentang = if self.makan(&Tok::Titik2) || self.makan(&Tok::Titik2Sama) {
                    let inklusif = matches!(&self.tok[self.pos - 1].0, Tok::Titik2Sama);
                    Some((self.ekspresi(), inklusif))
                } else {
                    None
                };
                self.larang_struk_lit -= 1;
                let badan = self.blok();
                match rentang {
                    Some((akhir, inklusif)) => Some(Stmt::UntukRentang { nama, dari: awal, sampai: akhir, inklusif, badan, span: sp }),
                    None => Some(Stmt::UntukIsi { nama, wadah: awal, badan, span: sp }),
                }
            }
            Tok::Kembali => {
                self.maju();
                let nilai = if matches!(self.intip(), Tok::TitikKoma | Tok::RBrace) {
                    None
                } else {
                    Some(self.ekspresi())
                };
                self.makan(&Tok::TitikKoma);
                Some(Stmt::Kembali(nilai, sp))
            }
            Tok::Tegas => {
                self.maju();
                let kond = self.ekspresi();
                let pesan = if self.makan(&Tok::Koma) {
                    if let Tok::Teks(t) = self.intip().clone() {
                        self.maju();
                        Some(t)
                    } else {
                        None
                    }
                } else {
                    None
                };
                self.makan(&Tok::TitikKoma);
                Some(Stmt::Tegas { kond, pesan, span: sp })
            }
            Tok::Henti => {
                self.maju();
                self.makan(&Tok::TitikKoma);
                Some(Stmt::Henti(sp))
            }
            Tok::Lanjut => {
                self.maju();
                self.makan(&Tok::TitikKoma);
                Some(Stmt::Lanjut(sp))
            }
            Tok::LBrace => Some(Stmt::Blok(self.blok())),
            Tok::Fungsi => {
                let fsp = self.intip_span();
                self.diag.push(Diagnostik::galat(
                    self.berkas,
                    fsp,
                    "fungsi bersarang tidak didukung; pindahkan ke tingkat teratas".into(),
                    "E0109",
                ));
                self.lompat_ke_batas();
                None
            }
            _ => {
                let e = self.ekspresi();
                if self.makan(&Tok::Sama) {
                    let nilai = self.ekspresi();
                    self.makan(&Tok::TitikKoma);
                    Some(Stmt::Tugas { target: e, nilai, span: sp })
                } else {
                    self.makan(&Tok::TitikKoma);
                    Some(Stmt::Ekspresi(e))
                }
            }
        }
    }

    fn eks_bil(&mut self, v: i64, sp: Span) -> Expr {
        Expr { id: self.id_baru(), span: sp, kind: ExprKind::Bil(v) }
    }

    fn kondisi(&mut self) -> Expr {
        self.larang_struk_lit += 1;
        let e = self.ekspresi();
        self.larang_struk_lit -= 1;
        e
    }

    // ---------- ekspresi ----------

    fn ekspresi(&mut self) -> Expr {
        self.atau()
    }

    fn atau(&mut self) -> Expr {
        let mut kiri = self.dan();
        loop {
            if self.intip() == &Tok::Atau {
                let sp = self.maju().1;
                let kanan = self.dan();
                let span = kiri.span.gabung(&kanan.span).gabung(&sp);
                kiri = Expr { id: self.id_baru(), span, kind: ExprKind::Biner(BinOp::Atau, Box::new(kiri), Box::new(kanan)) };
            } else {
                return kiri;
            }
        }
    }

    fn dan(&mut self) -> Expr {
        let mut kiri = self.banding();
        loop {
            if self.intip() == &Tok::Dan {
                let sp = self.maju().1;
                let kanan = self.banding();
                let span = kiri.span.gabung(&kanan.span).gabung(&sp);
                kiri = Expr { id: self.id_baru(), span, kind: ExprKind::Biner(BinOp::Dan, Box::new(kiri), Box::new(kanan)) };
            } else {
                return kiri;
            }
        }
    }

    fn banding(&mut self) -> Expr {
        let mut kiri = self.tambah();
        loop {
            let op = match self.intip() {
                Tok::SamaSama => BinOp::Sama,
                Tok::TidakSama => BinOp::TidakSama,
                Tok::Kecil => BinOp::Kecil,
                Tok::KecilSama => BinOp::KecilSama,
                Tok::Besar => BinOp::Besar,
                Tok::BesarSama => BinOp::BesarSama,
                _ => return kiri,
            };
            let sp = self.maju().1;
            let kanan = self.tambah();
            let span = kiri.span.gabung(&kanan.span).gabung(&sp);
            kiri = Expr { id: self.id_baru(), span, kind: ExprKind::Biner(op, Box::new(kiri), Box::new(kanan)) };
        }
    }

    fn tambah(&mut self) -> Expr {
        let mut kiri = self.kali();
        loop {
            let op = match self.intip() {
                Tok::Plus => BinOp::Tambah,
                Tok::Minus => BinOp::Kurang,
                _ => return kiri,
            };
            let sp = self.maju().1;
            let kanan = self.kali();
            let span = kiri.span.gabung(&kanan.span).gabung(&sp);
            kiri = Expr { id: self.id_baru(), span, kind: ExprKind::Biner(op, Box::new(kiri), Box::new(kanan)) };
        }
    }

    fn kali(&mut self) -> Expr {
        let mut kiri = self.uner();
        loop {
            let op = match self.intip() {
                Tok::Bintang => BinOp::Kali,
                Tok::GarisMiring => BinOp::Bagi,
                Tok::Persen => BinOp::Sisa,
                _ => return kiri,
            };
            let sp = self.maju().1;
            let kanan = self.uner();
            let span = kiri.span.gabung(&kanan.span).gabung(&sp);
            kiri = Expr { id: self.id_baru(), span, kind: ExprKind::Biner(op, Box::new(kiri), Box::new(kanan)) };
        }
    }

    fn uner(&mut self) -> Expr {
        let sp = self.intip_span();
        match self.intip().clone() {
            Tok::Minus => {
                self.maju();
                let dalam = self.uner();
                let span = sp.gabung(&dalam.span);
                Expr { id: self.id_baru(), span, kind: ExprKind::Uner(UnOp::Neg, Box::new(dalam)) }
            }
            Tok::TandaSeru | Tok::Bukan => {
                self.maju();
                let dalam = self.uner();
                let span = sp.gabung(&dalam.span);
                Expr { id: self.id_baru(), span, kind: ExprKind::Uner(UnOp::Bukan, Box::new(dalam)) }
            }
            Tok::Amp => {
                self.maju();
                let ubah = self.makan(&Tok::Ubah);
                let dalam = self.uner();
                let span = sp.gabung(&dalam.span);
                Expr { id: self.id_baru(), span, kind: ExprKind::Alamat { ubah, dasar: Box::new(dalam) } }
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Expr {
        let mut e = self.utama();
        loop {
            match self.intip().clone() {
                Tok::LPar => {
                    // panggilan hanya sah untuk nama fungsi
                    let sp = self.maju().1;
                    let args = self.argumen();
                    let (nama, span) = match &e.kind {
                        ExprKind::Var(n) => (n.clone(), e.span.gabung(&sp)),
                        _ => {
                            self.diag.push(Diagnostik::galat(
                                self.berkas,
                                e.span,
                                "hanya nama fungsi yang dapat dipanggil".into(),
                                "E0110",
                            ));
                            (String::new(), e.span.gabung(&sp))
                        }
                    };
                    e = Expr { id: self.id_baru(), span, kind: ExprKind::Panggil { nama, args } };
                }
                Tok::LBracket => {
                    let sp = self.maju().1;
                    let idx = self.ekspresi();
                    let akhir = self.harap(Tok::RBracket, "indeks").unwrap_or(sp);
                    let span = e.span.gabung(&akhir);
                    e = Expr { id: self.id_baru(), span, kind: ExprKind::Indeks { dasar: Box::new(e), indeks: Box::new(idx) } };
                }
                Tok::Titik => {
                    self.maju();
                    let (nama, nsp) = match self.harap_ident("nama medan") {
                        Some(v) => v,
                        None => return e,
                    };
                    if self.intip() == &Tok::LPar {
                        self.maju();
                        let args = self.argumen();
                        let span = e.span.gabung(&nsp);
                        let dasar = e;
                        let panggil = Expr {
                            id: self.id_baru(),
                            span,
                            kind: ExprKind::Panggil { nama: format!("__metode__{}", nama), args: vec![dasar.clone()].into_iter().chain(args).collect() },
                        };
                        e = panggil;
                    } else {
                        let span = e.span.gabung(&nsp);
                        e = Expr { id: self.id_baru(), span, kind: ExprKind::Medan { dasar: Box::new(e), nama } };
                    }
                }
                Tok::Sebagai => {
                    self.maju();
                    let ty = self.tipe();
                    let span = e.span;
                    e = Expr { id: self.id_baru(), span, kind: ExprKind::Sebagai { nilai: Box::new(e), ty } };
                }
                _ => return e,
            }
        }
    }

    fn argumen(&mut self) -> Vec<Expr> {
        let mut args = Vec::new();
        if self.makan(&Tok::RPar) {
            return args;
        }
        loop {
            args.push(self.ekspresi());
            if !self.makan(&Tok::Koma) {
                break;
            }
            if self.intip() == &Tok::RPar {
                break;
            }
        }
        self.harap(Tok::RPar, "akhir argumen");
        args
    }

    fn utama(&mut self) -> Expr {
        let sp = self.intip_span();
        match self.intip().clone() {
            Tok::Bil(v) => {
                self.maju();
                Expr { id: self.id_baru(), span: sp, kind: ExprKind::Bil(v) }
            }
            Tok::Char(c) => {
                self.maju();
                Expr { id: self.id_baru(), span: sp, kind: ExprKind::Huruf(c) }
            }
            Tok::Pecah(v) => {
                self.maju();
                Expr { id: self.id_baru(), span: sp, kind: ExprKind::Pecah(v) }
            }
            Tok::Teks(s) => {
                self.maju();
                Expr { id: self.id_baru(), span: sp, kind: ExprKind::Teks(s) }
            }
            Tok::Benar => {
                self.maju();
                Expr { id: self.id_baru(), span: sp, kind: ExprKind::Log(true) }
            }
            Tok::Salah => {
                self.maju();
                Expr { id: self.id_baru(), span: sp, kind: ExprKind::Log(false) }
            }
            Tok::LPar => {
                self.maju();
                let e = self.ekspresi();
                self.harap(Tok::RPar, "kurung tutup");
                e
            }
            Tok::LBracket => {
                self.maju();
                let mut isi = Vec::new();
                if !self.makan(&Tok::RBracket) {
                    loop {
                        isi.push(self.ekspresi());
                        if !self.makan(&Tok::Koma) {
                            break;
                        }
                        if self.intip() == &Tok::RBracket {
                            break;
                        }
                    }
                    let akhir = self.harap(Tok::RBracket, "akhir larik").unwrap_or(sp);
                    return Expr { id: self.id_baru(), span: sp.gabung(&akhir), kind: ExprKind::ArrayLit(isi) };
                }
                Expr { id: self.id_baru(), span: sp, kind: ExprKind::ArrayLit(isi) }
            }
            Tok::Ident(nama) => {
                self.maju();
                if self.intip() == &Tok::DuaTitik2 {
                    self.maju();
                    let (varian, vsp) = match self.harap_ident("nama varian") {
                        Some(v) => v,
                        None => return self.eks_bil(0, sp),
                    };
                    return Expr { id: self.id_baru(), span: sp.gabung(&vsp), kind: ExprKind::Varian { enumerasi: nama, varian } };
                }
                if self.intip() == &Tok::LBrace && self.larang_struk_lit == 0 {
                    let awal = self.intip_span();
                    self.maju();
                    let mut medan = Vec::new();
                    if !self.makan(&Tok::RBrace) {
                        loop {
                            let Some((mn, _)) = self.harap_ident("nama medan") else {
                                self.lompat_ke_batas();
                                break;
                            };
                            let nilai = if self.makan(&Tok::TitikDua) {
                                self.ekspresi()
                            } else if self.makan(&Tok::Sama) {
                                self.ekspresi()
                            } else {
                                let sp2 = self.intip_span();
                                self.diag.push(Diagnostik::galat(
                                    self.berkas,
                                    sp2,
                                    format!("medan `{}` memerlukan `: nilai`", mn),
                                    "E0111",
                                ));
                                self.eks_bil(0, sp2)
                            };
                            medan.push((mn, nilai));
                            if !self.makan(&Tok::Koma) {
                                break;
                            }
                            if self.intip() == &Tok::RBrace {
                                break;
                            }
                        }
                        self.harap(Tok::RBrace, "akhir literal struk");
                    }
                    return Expr { id: self.id_baru(), span: sp.gabung(&awal), kind: ExprKind::StrukLit { nama, medan } };
                }
                Expr { id: self.id_baru(), span: sp, kind: ExprKind::Var(nama) }
            }
            lain => {
                self.diag.push(Diagnostik::galat(
                    self.berkas,
                    sp,
                    format!("mengharapkan ekspresi, tetapi menemukan {}", lain.sebutan()),
                    "E0112",
                ));
                self.maju();
                self.eks_bil(0, sp)
            }
        }
    }

    // ---------- tipe ----------

    fn tipe(&mut self) -> Ty {
        let sp = self.intip_span();
        match self.intip().clone() {
            Tok::Amp => {
                self.maju();
                let ubah = self.makan(&Tok::Ubah);
                let dalam = self.tipe();
                Ty::Ref { ubah, dalam: Box::new(dalam) }
            }
            Tok::LBracket => {
                self.maju();
                let dalam = self.tipe();
                if self.makan(&Tok::TitikKoma) {
                    let n = match self.intip().clone() {
                        Tok::Bil(v) if v >= 0 => {
                            self.maju();
                            v as usize
                        }
                        _ => {
                            let s2 = self.intip_span();
                            self.diag.push(Diagnostik::galat(
                                self.berkas,
                                s2,
                                "panjang larik haruslah angka bulat positif".into(),
                                "E0113",
                            ));
                            self.maju();
                            1
                        }
                    };
                    self.harap(Tok::RBracket, "akhir tipe larik");
                    Ty::Array(Box::new(dalam), n)
                } else {
                    self.harap(Tok::RBracket, "akhir tipe irisan");
                    Ty::Irisan(Box::new(dalam))
                }
            }
            Tok::Ident(nama) => {
                self.maju();
                let dasar = match nama.as_str() {
                    "bil" | "bl" => Ty::Bil,
                    "bil32" | "bl32" => Ty::Bil32,
                    "bil8" | "bl8" => Ty::Bil8,
                    "nat" | "nt" => Ty::Nat,
                    "nat8" | "nt8" => Ty::Nat8,
                    "pecah" | "pc" => Ty::Pecah,
                    "log" | "lg" => Ty::Log,
                    "huruf" | "hr" => Ty::Huruf,
                    "teks" | "tk" => Ty::Teks,
                    "kosong" | "ks" => Ty::Kosong,
                    "Daftar" | "Dft" | "daftar" | "dft" => {
                        if self.makan(&Tok::Kecil) {
                            let dalam = self.tipe();
                            self.harap(Tok::Besar, "akhir argumen tipe Daftar");
                            Ty::Daftar(Box::new(dalam))
                        } else {
                            Ty::Nama(nama.clone())
                        }
                    }
                    _ => Ty::Nama(nama.clone()),
                };
                dasar
            }
            lain => {
                self.diag.push(Diagnostik::galat(
                    self.berkas,
                    sp,
                    format!("mengharapkan tipe, tetapi menemukan {}", lain.sebutan()),
                    "E0114",
                ));
                self.maju();
                Ty::Bil
            }
        }
    }
}
