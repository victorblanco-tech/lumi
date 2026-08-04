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

const EXTENDED_BUTTON_ROLE_IDS: [&str; 24] = [
    "buildup-2",
    "breakdown-3",
    "buildup-3",
    "drop",
    "synth",
    "bridge",
    "pre-drop",
    "intro-outro",
    "breakdown-1",
    "buildup-1",
    "breakdown-2",
    "buildup-2",
    "breakdown-3",
    "buildup-3",
    "synth",
    "drop",
    "bridge",
    "pre-drop",
    "intro-outro",
    "breakdown-1",
    "buildup-1",
    "drop",
    "synth",
    "bridge",
];

const BUTTONS_PER_BANK: usize = 32;

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
    let resolved_mappings = themes
        .iter()
        .enumerate()
        .map(|(bank_index, theme)| {
            (1..=BUTTONS_PER_BANK)
                .map(|button_number| {
                    let mapping_id = format!("mapping-{button_number}");
                    existing
                        .cells()
                        .iter()
                        .find(|cell| {
                            cell.theme_id() == theme.id()
                                && cell.variant_id().as_str() == mapping_id
                        })
                        .map_or_else(
                            || {
                                let role_id = default_role_id(bank_index, button_number);
                                (
                                    role_id.to_owned(),
                                    default_autoloop_name(theme, role_id, button_number),
                                )
                            },
                            |cell| {
                                (
                                    cell.role_id().as_str().to_owned(),
                                    cell.display_name().to_owned(),
                                )
                            },
                        )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let known_roles = phrase_roles
        .roles()
        .iter()
        .map(|role| role.id().as_str())
        .collect::<std::collections::HashSet<_>>();
    if resolved_mappings
        .iter()
        .flatten()
        .any(|(role_id, _)| !known_roles.contains(role_id.as_str()))
    {
        return Err(AutoloopDefaultsError::MissingDefaultRole);
    }
    let mut variant_keys = resolved_mappings
        .iter()
        .flat_map(|bank| bank.iter().enumerate())
        .map(|(index, (role_id, _))| (role_id.clone(), index + 1))
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
        for (button_index, (role_id, display_name)) in
            resolved_mappings[bank_index].iter().enumerate()
        {
            let button_number = button_index + 1;
            let variant_id = VariantId::try_new(format!("mapping-{button_number}"))?;
            cells.push(AutoloopMatrixCell::try_new(
                theme.id(),
                lumi_library::PhraseRoleId::try_new(role_id.clone())?,
                variant_id,
                AutoloopEntryId::try_new(format!(
                    "theme-{}--mapping-{button_number}",
                    theme.id().value(),
                ))?,
                display_name.clone(),
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

fn default_role_id(bank_index: usize, button_number: usize) -> &'static str {
    if button_number <= 8 {
        BUTTON_ROLE_IDS[bank_index][button_number - 1]
    } else {
        EXTENDED_BUTTON_ROLE_IDS[button_number - 9]
    }
}

fn default_autoloop_name(theme: &AutoloopTheme, role_id: &str, button_number: usize) -> String {
    let role_name = role_id.replace('-', " ").to_uppercase();
    format!("{role_name} · {} {button_number}", theme.display_name())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phrase_role_defaults::seeded_phrase_role_catalog;
    use lumi_library::{PhraseRoleCatalog, PhraseRoleId};

    #[test]
    fn version_two_upgrade_preserves_existing_buttons_and_adds_thirty_two_per_bank()
    -> Result<(), Box<dyn std::error::Error>> {
        let empty_roles = PhraseRoleCatalog::try_new(0, 0, Vec::new(), Vec::new())?;
        let phrase_roles = seeded_phrase_role_catalog(&empty_roles)?;
        let empty_catalog = AutoloopCatalog::try_new(0, 0, Vec::new(), Vec::new(), Vec::new())?;
        let seeded = seeded_autoloop_catalog(&empty_catalog, &phrase_roles)?;
        let customized = seeded
            .rename_theme(ThemeId::new(1), "My First Bank")?
            .set_mapping(
                ThemeId::new(1),
                VariantId::try_new("mapping-1")?,
                PhraseRoleId::try_new("intro-outro")?,
                Some("MY EXACT AUTOLOOP".to_owned()),
            )?;
        let first_page_cells = customized
            .cells()
            .iter()
            .filter(|cell| mapping_number(cell.variant_id()) <= 8)
            .cloned()
            .collect::<Vec<_>>();
        let first_page_variants = customized
            .variants()
            .iter()
            .filter(|variant| {
                first_page_cells.iter().any(|cell| {
                    cell.role_id() == variant.role_id() && cell.variant_id() == variant.id()
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        let version_two = AutoloopCatalog::try_new(
            7,
            2,
            customized.themes().to_vec(),
            first_page_variants,
            first_page_cells,
        )?;

        let upgraded = seeded_autoloop_catalog(&version_two, &phrase_roles)?;

        assert_eq!(upgraded.revision(), 8);
        assert_eq!(upgraded.defaults_version(), 3);
        assert_eq!(upgraded.cells().len(), 128);
        assert_eq!(upgraded.themes()[0].display_name(), "My First Bank");
        let first = upgraded
            .cells()
            .iter()
            .find(|cell| {
                cell.theme_id() == ThemeId::new(1) && cell.variant_id().as_str() == "mapping-1"
            })
            .ok_or("first mapping missing")?;
        assert_eq!(first.role_id().as_str(), "intro-outro");
        assert_eq!(first.display_name(), "MY EXACT AUTOLOOP");
        assert!(upgraded.cells().iter().any(|cell| {
            cell.theme_id() == ThemeId::new(4) && cell.variant_id().as_str() == "mapping-32"
        }));
        Ok(())
    }

    fn mapping_number(id: &VariantId) -> u16 {
        id.as_str()
            .strip_prefix("mapping-")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    }
}
