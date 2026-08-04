use lumi_domain::ThemeId;
use lumi_library::{
    AUTOLOOP_CATALOG_DEFAULTS_VERSION, AutoloopCatalog, AutoloopCatalogError, AutoloopEntryId,
    AutoloopMatrixCell, AutoloopTheme, AutoloopVariant, PhraseRoleCatalog, VariantId,
};
use thiserror::Error;

const THEME_NAMES: [&str; 4] = ["Electric Bloom", "Deep Ocean", "Solar Flare", "Ultraviolet"];

const BUTTON_ROLE_IDS: [[&str; 8]; 4] = [
    [
        "intro-outro",
        "breakdown-1",
        "buildup-1",
        "drop",
        "synth",
        "breakdown-2",
        "buildup-3",
        "pre-drop",
    ],
    [
        "intro-outro",
        "breakdown-1",
        "buildup-1",
        "drop",
        "synth",
        "bridge",
        "pre-drop",
        "breakdown-2",
    ],
    [
        "intro-outro",
        "breakdown-1",
        "buildup-1",
        "drop",
        "synth",
        "bridge",
        "breakdown-3",
        "buildup-3",
    ],
    [
        "intro-outro",
        "breakdown-1",
        "buildup-1",
        "drop",
        "synth",
        "bridge",
        "buildup-2",
        "pre-drop",
    ],
];

pub fn seeded_autoloop_catalog(
    existing: &AutoloopCatalog,
    phrase_roles: &PhraseRoleCatalog,
) -> Result<AutoloopCatalog, AutoloopDefaultsError> {
    if existing.defaults_version() >= AUTOLOOP_CATALOG_DEFAULTS_VERSION {
        existing.validate_roles(phrase_roles)?;
        return Ok(existing.clone());
    }
    let themes = if existing.revision() == 0 {
        THEME_NAMES
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
            .collect::<Result<Vec<_>, AutoloopDefaultsError>>()?
    } else {
        existing.themes().to_vec()
    };
    let known_roles = phrase_roles
        .roles()
        .iter()
        .map(|role| role.id().as_str())
        .collect::<std::collections::HashSet<_>>();
    if BUTTON_ROLE_IDS
        .iter()
        .flatten()
        .any(|role_id| !known_roles.contains(role_id))
    {
        return Err(AutoloopDefaultsError::MissingDefaultRole);
    }
    let mut variant_keys = BUTTON_ROLE_IDS
        .iter()
        .flat_map(|bank| bank.iter().enumerate())
        .map(|(index, role_id)| ((*role_id).to_owned(), index + 1))
        .collect::<Vec<_>>();
    variant_keys.sort();
    variant_keys.dedup();
    let mut role_orders = std::collections::HashMap::<String, u16>::new();
    let variants = variant_keys
        .iter()
        .map(|(role_id, button_number)| {
            let order = role_orders
                .entry(role_id.clone())
                .and_modify(|value| *value += 1)
                .or_insert(1);
            Ok(AutoloopVariant::try_new(
                lumi_library::PhraseRoleId::try_new(role_id.clone())?,
                VariantId::try_new(format!("mapping-{button_number}"))?,
                format!("Output mapping {button_number}"),
                *order,
                false,
            )?)
        })
        .collect::<Result<Vec<_>, AutoloopDefaultsError>>()?;
    let mut cells = Vec::new();
    for (bank_index, theme) in themes.iter().enumerate() {
        for (button_index, role_id) in BUTTON_ROLE_IDS[bank_index].iter().enumerate() {
            let button_number = button_index + 1;
            let variant_id = VariantId::try_new(format!("mapping-{button_number}"))?;
            let role_name = phrase_roles
                .roles()
                .iter()
                .find(|role| role.id().as_str() == *role_id)
                .map_or(*role_id, |role| role.display_name());
            cells.push(AutoloopMatrixCell::try_new(
                theme.id(),
                lumi_library::PhraseRoleId::try_new((*role_id).to_owned())?,
                variant_id,
                AutoloopEntryId::try_new(format!(
                    "theme-{}--mapping-{button_number}",
                    theme.id().value(),
                ))?,
                format!("{} · {} {button_number}", theme.display_name(), role_name),
            )?);
        }
    }
    let revision = if existing.revision() == 0 {
        1
    } else {
        existing
            .revision()
            .checked_add(1)
            .ok_or(AutoloopDefaultsError::Overflow)?
    };
    let catalog = AutoloopCatalog::try_new(
        revision,
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
    #[error("the Phrase Role defaults do not contain every SoundSwitch demo mapping role")]
    MissingDefaultRole,
}
