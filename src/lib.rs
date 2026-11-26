//! Kompiler Nusa: bahasa Indonesia yang aman memori dan bertipe statis.

pub mod ast;
pub mod codegen;
pub mod diag;
pub mod driver;
pub mod interp;
pub mod lexer;
pub mod parser;
pub mod types;
pub mod verify;

use diag::{Diag, Hasil};

pub struct HasilAnalisis {
    pub program: ast::Program,
    pub info: types::Info,
}

pub fn analisis(sumber: &str) -> Hasil<HasilAnalisis> {
    let tokens = lexer::lex(sumber)?;
    let program = parser::parse(tokens)?;
    let info = types::periksa(&program)?;
    Ok(HasilAnalisis { program, info })
}

pub fn kompilasi_ir(
    sumber: &str,
    opsi: codegen::Opsi,
) -> Hasil<(codegen::Keluaran, HasilAnalisis)> {
    let a = analisis(sumber)?;
    let keluaran = codegen::bangkitkan(&a.program, &a.info, opsi)?;
    Ok((keluaran, a))
}

pub fn galat_dari_pesan(pesan: impl Into<String>) -> Diag {
    Diag::baru("NUSA-X001", pesan)
}
