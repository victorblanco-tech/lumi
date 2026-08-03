use lumi_domain::ThemeId;
use lumi_library::{
    AUTOLOOP_CATALOG_DEFAULTS_VERSION, AutoloopCatalog, AutoloopCatalogError, AutoloopEntryId,
    AutoloopMatrixCell, AutoloopTheme, AutoloopVariant, PhraseRoleCatalog, VariantId,
};
use thiserror::Error;

const THEME_NAMES: [&str; 4] = ["Electric Bloom", "Deep Ocean", "Solar Flare", "Ultraviolet"];

const TWO_VARIANT_ROLES: [&str; 8] = [
    "breakdown-1",
    "breakdown-2",
    "breakdown-3",
    "synth",
    "buildup-1",
    "buildup-2",
    "buildup-3",
    "drop",
];

pub fn seeded_autoloop_catalog(
    existing: &AutoloopCatalog,
    phrase_roles: &PhraseRoleCatalog,
) -> Result<AutoloopCatalog, AutoloopDefaultsError> {
    if existing.defaults_version() >= AUTOLOOP_CATALOG_DEFAULTS_VERSION {
        existing.validate_roles(phrase_roles)?;
        return Ok(existing.clone());
    }
    if existing.revision() != 0 {
        return Err(AutoloopDefaultsError::UnexpectedExistingCatalog);
    }
    let themes = THEME_NAMES
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            Ok(AutoloopTheme::try_new(
                ThemeId::new(
                    u64::try_from(index + 1).map_err(|_| AutoloopDefaultsError::Overflow)?,
                ),
                name,
                u16::try_from(index + 1).map_err(|_| AutoloopDefaultsError::Overflow)?,
            )?)
        })
        .collect::<Result<Vec<_>, AutoloopDefaultsError>>()?;
    let mut variants = Vec::new();
    for role in phrase_roles.roles() {
        variants.push(AutoloopVariant::try_new(
            role.id().clone(),
            VariantId::try_new("variant-1")?,
            "Variant 1",
            1,
            false,
        )?);
        if TWO_VARIANT_ROLES.contains(&role.id().as_str()) {
            variants.push(AutoloopVariant::try_new(
                role.id().clone(),
                VariantId::try_new("variant-2")?,
                "Variant 2",
                2,
                false,
            )?);
        }
    }
    variants.sort_by(|left, right| {
        left.role_id()
            .cmp(right.role_id())
            .then_with(|| left.sort_order().cmp(&right.sort_order()))
    });
    let mut cells = Vec::new();
    for theme in &themes {
        for variant in &variants {
            let role_name = phrase_roles
                .roles()
                .iter()
                .find(|role| role.id() == variant.role_id())
                .map_or(variant.role_id().as_str(), |role| role.display_name());
            cells.push(AutoloopMatrixCell::try_new(
                theme.id(),
                variant.role_id().clone(),
                variant.id().clone(),
                AutoloopEntryId::try_new(format!(
                    "theme-{}--{}--{}",
                    theme.id().value(),
                    variant.role_id().as_str(),
                    variant.id().as_str()
                ))?,
                format!(
                    "{} · {} · {}",
                    theme.display_name(),
                    role_name,
                    variant.display_name()
                ),
            )?);
        }
    }
    let catalog = AutoloopCatalog::try_new(
        1,
        AUTOLOOP_CATALOG_DEFAULTS_VERSION,
        themes,
        variants,
        cells,
    )?;
    catalog.validate_roles(phrase_roles)?;
    Ok(catalog)
}

#[derive(Debug, Error)]
pub enum AutoloopDefaultsError {
    #[error("invalid Autoloop defaults: {0}")]
    Catalog(#[from] AutoloopCatalogError),
    #[error("invalid Autoloop defaults identifier: {0}")]
    Identifier(#[from] lumi_library::TextIdentifierError),
    #[error("Autoloop defaults arithmetic overflow")]
    Overflow,
    #[error("an unversioned Autoloop catalog unexpectedly contains user data")]
    UnexpectedExistingCatalog,
}
