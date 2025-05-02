//! Module for trait sealing.

/// Sealed [`super::Configuration`] trait.
pub trait ConfigurationSealed {}

impl<C: ConfigurationSealed> ConfigurationSealed for &C {}
