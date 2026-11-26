//! Pohon sintaks abstrak (AST) bahasa Nusa.

use crate::diag::Span;
use std::fmt;

pub type ExprId = u32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tipe {
    Bil,
    Des,
    Log,
    Teks,
    Daftar(Box<Tipe>),
    Kosong,
}

impl Tipe {
    pub fn nama(&self) -> String {
        match self {
            Tipe::Bil => "bil".into(),
            Tipe::Des => "des".into(),
            Tipe::Log => "log".into(),
            Tipe::Teks => "teks".into(),
            Tipe::Daftar(t) => format!("daftar<{}>", t.nama()),
            Tipe::Kosong => "kosong".into(),
        }
    }

    pub fn adalah_heap(&self) -> bool {
        matches!(self, Tipe::Teks | Tipe::Daftar(_))
    }
}

impl fmt::Display for Tipe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.nama())
    }
}

#[derive(Clone, Debug)]
pub struct Param {
    pub nama: String,
    pub tipe: Tipe,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Fungsi {
    pub nama: String,
    pub params: Vec<Param>,
    pub hasil: Tipe,
    pub syarat: Vec<Expr>,
    pub jamin: Vec<Expr>,
    pub tubuh: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub fungsi: Vec<Fungsi>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Tidak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Tambah,
    Kurang,
    Kali,
    Bagi,
    Mod,
    SamaSama,
    TidakSama,
    KurangDari,
    KurangSama,
    LebihDari,
    LebihSama,
    Dan,
    Atau,
}

impl BinOp {
    pub fn simbol(&self) -> &'static str {
        match self {
            BinOp::Tambah => "+",
            BinOp::Kurang => "-",
            BinOp::Kali => "*",
            BinOp::Bagi => "/",
            BinOp::Mod => "%",
            BinOp::SamaSama => "==",
            BinOp::TidakSama => "!=",
            BinOp::KurangDari => "<",
            BinOp::KurangSama => "<=",
            BinOp::LebihDari => ">",
            BinOp::LebihSama => ">=",
            BinOp::Dan => "dan",
            BinOp::Atau => "atau",
        }
    }

    pub fn adalah_perbandingan(&self) -> bool {
        matches!(
            self,
            BinOp::SamaSama
                | BinOp::TidakSama
                | BinOp::KurangDari
                | BinOp::KurangSama
                | BinOp::LebihDari
                | BinOp::LebihSama
        )
    }
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub id: ExprId,
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Bil(i64),
    Des(f64),
    Teks(String),
    Log(bool),
    Nama(String),
    Daftar(Vec<Expr>),
    Unary(UnOp, Box<Expr>),
    Biner(BinOp, Box<Expr>, Box<Expr>),
    Panggil(String, Vec<Expr>),
    Metode(Box<Expr>, String, Vec<Expr>),
    Indeks(Box<Expr>, Box<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TugasOp {
    Isi,
    Tambah,
    Kurang,
    Kali,
    Bagi,
    Mod,
}

#[derive(Clone, Debug)]
pub enum Iterasi {
    Rentang {
        mulai: Expr,
        akhir: Expr,
        inklusif: bool,
    },
    Setiap(Expr),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Tetap {
        nama: String,
        tipe: Option<Tipe>,
        nilai: Expr,
        span: Span,
    },
    Var {
        nama: String,
        tipe: Option<Tipe>,
        nilai: Expr,
        span: Span,
    },
    Tugas {
        target: Expr,
        op: TugasOp,
        nilai: Expr,
        span: Span,
    },
    Ekspresi(Expr),
    Jika {
        kondisi: Expr,
        maka: Vec<Stmt>,
        lainnya: Option<Vec<Stmt>>,
        span: Span,
    },
    Selama {
        kondisi: Expr,
        invarian: Vec<Expr>,
        tubuh: Vec<Stmt>,
        span: Span,
    },
    Untuk {
        nama: String,
        nama_span: Span,
        iterasi: Iterasi,
        invarian: Vec<Expr>,
        tubuh: Vec<Stmt>,
        span: Span,
    },
    Kembalikan {
        nilai: Option<Expr>,
        span: Span,
    },
    Tegaskan {
        kondisi: Expr,
        span: Span,
    },
    Hentikan {
        span: Span,
    },
    Lanjutkan {
        span: Span,
    },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Tetap { span, .. }
            | Stmt::Var { span, .. }
            | Stmt::Tugas { span, .. }
            | Stmt::Jika { span, .. }
            | Stmt::Selama { span, .. }
            | Stmt::Untuk { span, .. }
            | Stmt::Kembalikan { span, .. }
            | Stmt::Tegaskan { span, .. }
            | Stmt::Hentikan { span }
            | Stmt::Lanjutkan { span } => *span,
            Stmt::Ekspresi(e) => e.span,
        }
    }
}

/// Mencetak pohon ekspresi kembali menjadi bentuk sumber (untuk pesan).
pub fn cetak_ekspr(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Bil(n) => n.to_string(),
        ExprKind::Des(n) => {
            if n.fract() == 0.0 {
                format!("{:.1}", n)
            } else {
                n.to_string()
            }
        }
        ExprKind::Teks(s) => format!("{:?}", s),
        ExprKind::Log(b) => if *b { "benar" } else { "salah" }.to_string(),
        ExprKind::Nama(n) => n.clone(),
        ExprKind::Daftar(xs) => format!(
            "[{}]",
            xs.iter().map(cetak_ekspr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Unary(op, e) => match op {
            UnOp::Neg => format!("-{}", cetak_ekspr(e)),
            UnOp::Tidak => format!("bukan {}", cetak_ekspr(e)),
        },
        ExprKind::Biner(op, a, b) => {
            format!("{} {} {}", cetak_ekspr(a), op.simbol(), cetak_ekspr(b))
        }
        ExprKind::Panggil(n, args) => format!(
            "{}({})",
            n,
            args.iter().map(cetak_ekspr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Metode(res, n, args) => format!(
            "{}.{}({})",
            cetak_ekspr(res),
            n,
            args.iter().map(cetak_ekspr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Indeks(a, i) => format!("{}[{}]", cetak_ekspr(a), cetak_ekspr(i)),
    }
}
