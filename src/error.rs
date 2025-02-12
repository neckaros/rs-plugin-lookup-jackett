use serde::Serialize;

use crate::JackettResult;

#[derive(Debug, Serialize, strum_macros::AsRefStr)]
pub enum JackettError {

	NoLink(JackettResult)

}

// region:    --- Error Boilerplate

impl core::fmt::Display for JackettError {
	fn fmt(
		&self,
		fmt: &mut core::fmt::Formatter,
	) -> core::result::Result<(), core::fmt::Error> {
		write!(fmt, "{self:?}")
	}
}

impl std::error::Error for JackettError {}