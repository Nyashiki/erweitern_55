#[macro_use]
extern crate lazy_static;
extern crate bitintr;
extern crate pyo3;
extern crate rand;

pub mod bitboard;
pub mod r#move;
pub mod position;
pub mod types;

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

use position::*;

#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pymodule]
fn minishogilib(_py: Python, m: &PyModule) -> PyResult<()> {
    bitboard::init();

    m.add_wrapped(wrap_pyfunction!(version))?;
    m.add_class::<Position>()?;

    Ok(())
}
