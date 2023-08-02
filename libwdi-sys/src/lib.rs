// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2022-2023 1BitSquared <info@1bitsquared.com>
// SPDX-FileContributor: Written by Mikaela Szekely <mikaela.szekely@qyriad.me>

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[cfg(not(feature = "dynamic-bindgen"))]
mod bindings;
#[cfg(not(feature = "dynamic-bindgen"))]
pub use bindings::*;

#[cfg(feature = "dynamic-bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
