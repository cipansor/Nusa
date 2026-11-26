//! Pohon sintaks abstrak bahasa NUSA.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub baris: u32,
    pub kol: u32,
    pub panjang: u32,
}

impl Span {
    pub fn baru(baris: u32, kol: u32, panjang: u32) -> Self {
        Span {
            baris,
            kol,
            panjang,
        }
    }
    pub fn gabung(&self, lain: &Span) -> Span {
        let akhir = (lain.baris, lain.kol + lain.panjang);
        let akhir_self = (self.baris, self.kol + self.panjang);
        let (b, k) = if akhir > akhir_self {
            akhir
        } else {
            akhir_self
        };
        // panjang dihitung per baris; cukup untuk penunjuk satu baris.
        let panjang = if b == self.baris && b == lain.baris {
            k.saturating_sub(self.kol)
        } else {
            self.panjang
        };
        Span::baru(self.baris, self.kol, panjang)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Bil,                   // i64
    Bil32,                 // i32
    Bil8,                  // i8
    Nat,                   // u64
    Nat8,                  // u8
    Pecah,                 // f64
    Log,                   // bool
    Huruf,                 // u8 (huruf/byte)
    Teks,                  // irisan byte immutable (&str)
    Kosong,                // unit
    Array(Box<Ty>, usize), // [T; N]
    Irisan(Box<Ty>),       // [T]  (irisan/&[T])
    Ref { ubah: bool, dalam: Box<Ty> },
    Daftar(Box<Ty>), // Daftar<T>
    Nama(String),    // struk / enumerasi
}

impl Ty {
    /// Tipe yang boleh disalin tanpa memindahkan kepemilikan.
    pub fn boleh_salin(&self) -> bool {
        match self {
            Ty::Bil
            | Ty::Bil32
            | Ty::Bil8
            | Ty::Nat
            | Ty::Nat8
            | Ty::Pecah
            | Ty::Log
            | Ty::Huruf
            | Ty::Teks
            | Ty::Kosong
            | Ty::Ref { .. } => true,
            Ty::Array(dalam, _) => dalam.boleh_salin(),
            _ => false,
        }
    }

    pub fn sebutan(&self) -> String {
        match self {
            Ty::Bil => "bil".into(),
            Ty::Bil32 => "bil32".into(),
            Ty::Bil8 => "bil8".into(),
            Ty::Nat => "nat".into(),
            Ty::Nat8 => "nat8".into(),
            Ty::Pecah => "pecah".into(),
            Ty::Log => "log".into(),
            Ty::Huruf => "huruf".into(),
            Ty::Teks => "teks".into(),
            Ty::Kosong => "kosong".into(),
            Ty::Array(t, n) => format!("[{}; {}]", t.sebutan(), n),
            Ty::Irisan(t) => format!("[{}]", t.sebutan()),
            Ty::Ref { ubah, dalam } => {
                if *ubah {
                    format!("&ubah {}", dalam.sebutan())
                } else {
                    format!("&{}", dalam.sebutan())
                }
            }
            Ty::Daftar(t) => format!("Daftar<{}>", t.sebutan()),
            Ty::Nama(n) => n.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Tambah,
    Kurang,
    Kali,
    Bagi,
    Sisa,
    Dan,
    Atau,
    Sama,
    TidakSama,
    Kecil,
    KecilSama,
    Besar,
    BesarSama,
    GeserKiri,
    GeserKanan,
}

impl BinOp {
    pub fn simbol(&self) -> &'static str {
        match self {
            BinOp::Tambah => "+",
            BinOp::Kurang => "-",
            BinOp::Kali => "*",
            BinOp::Bagi => "/",
            BinOp::Sisa => "%",
            BinOp::Dan => "dan",
            BinOp::Atau => "atau",
            BinOp::Sama => "==",
            BinOp::TidakSama => "!=",
            BinOp::Kecil => "<",
            BinOp::KecilSama => "<=",
            BinOp::Besar => ">",
            BinOp::BesarSama => ">=",
            BinOp::GeserKiri => "<<",
            BinOp::GeserKanan => ">>",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Bukan,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub id: u32,
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Bil(i64),
    Huruf(u8),
    Pecah(f64),
    Teks(String),
    Log(bool),
    Var(String),
    Biner(BinOp, Box<Expr>, Box<Expr>),
    Uner(UnOp, Box<Expr>),
    Panggil {
        nama: String,
        args: Vec<Expr>,
    },
    Indeks {
        dasar: Box<Expr>,
        indeks: Box<Expr>,
    },
    Medan {
        dasar: Box<Expr>,
        nama: String,
    },
    Alamat {
        ubah: bool,
        dasar: Box<Expr>,
    },
    Sebagai {
        nilai: Box<Expr>,
        ty: Ty,
    },
    StrukLit {
        nama: String,
        medan: Vec<(String, Expr)>,
    },
    Varian {
        enumerasi: String,
        varian: String,
    },
    ArrayLit(Vec<Expr>),
}

#[derive(Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Ikat {
        tetap: bool,
        nama: String,
        ty: Option<Ty>,
        nilai: Expr,
        span: Span,
    },
    Tugas {
        target: Expr,
        nilai: Expr,
        span: Span,
    },
    Jika {
        kond: Expr,
        maka: Block,
        lain: Option<Block>,
        span: Span,
    },
    Selama {
        kond: Expr,
        invarian: Vec<Expr>,
        badan: Block,
        span: Span,
    },
    UntukRentang {
        nama: String,
        dari: Expr,
        sampai: Expr,
        inklusif: bool,
        badan: Block,
        span: Span,
    },
    UntukIsi {
        nama: String,
        wadah: Expr,
        badan: Block,
        span: Span,
    },
    Kembali(Option<Expr>, Span),
    Tegas {
        kond: Expr,
        pesan: Option<String>,
        span: Span,
    },
    Ekspresi(Expr),
    Henti(Span),
    Lanjut(Span),
    Blok(Block),
    Takaman(Block),
    Cocok {
        nilai: Expr,
        kasus: Vec<(String, Block)>,
        lainnya: Option<Block>,
        span: Span,
    },
}

#[derive(Clone, Debug)]
pub struct Param {
    pub nama: String,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Fungsi {
    pub nama: String,
    pub params: Vec<Param>,
    pub hasil: Ty,
    pub perlu: Vec<Expr>,
    pub jamin: Vec<Expr>,
    pub badan: Block,
    pub span: Span,
    pub luar: bool,
}

#[derive(Clone, Debug)]
pub struct Struk {
    pub nama: String,
    pub medan: Vec<Param>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Enumerasi {
    pub nama: String,
    pub varian: Vec<String>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Alias {
    pub nama: String,
    pub ty: Ty,
    #[allow(dead_code)]
    pub span: Span,
}

#[derive(Clone, Debug, Default)]
pub struct Prog {
    pub fungsi: Vec<Fungsi>,
    pub struk: Vec<Struk>,
    pub enumerasi: Vec<Enumerasi>,
    pub alias: Vec<Alias>,
    pub impor: Vec<String>,
}
