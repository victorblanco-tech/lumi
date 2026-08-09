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
    // Some catalogs created by the first profile editor still carry defaults
    // version 1 even though later UI mutations expanded them to the 32-slot
    // SoundSwitch surface. Detect the persisted shape rather than trusting only
    // the seed version, while leaving the original eight-button catalogs on the
    // normal upgrade path.
    if existing.revision() != 0 && uses_row_major_sound_switch_surface(existing) {
        return transpose_sound_switch_slots(existing, phrase_roles);
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

/// Version 3 rendered catalog slots row-major in a four-column grid, while the
/// SoundSwitch learn/controller surface numbers each column top-to-bottom
/// (1...8, 9...16, 17...24, 25...32). Preserve every user-authored role,
/// display name, Theme and variant order while moving the stable mapping IDs to
/// the physical SoundSwitch slot at the same visual position.
fn transpose_sound_switch_slots(
    existing: &AutoloopCatalog,
    phrase_roles: &PhraseRoleCatalog,
) -> Result<AutoloopCatalog, AutoloopDefaultsError> {
    let mut cells = Vec::new();
    for cell in existing.cells() {
        if is_generated_sound_switch_default(cell, existing) {
            continue;
        }
        cells.push(AutoloopMatrixCell::try_new(
            cell.theme_id(),
            cell.role_id().clone(),
            transposed_mapping_id(cell.variant_id())?,
            cell.entry_id().clone(),
            cell.display_name().to_owned(),
        )?);
    }
    let referenced = cells
        .iter()
        .map(|cell| {
            (
                cell.role_id().as_str().to_owned(),
                cell.variant_id().as_str().to_owned(),
            )
        })
        .collect::<std::collections::HashSet<_>>();
    let mut role_orders = std::collections::HashMap::<String, u16>::new();
    let mut variants = Vec::new();
    for variant in existing.variants() {
        let id = transposed_mapping_id(variant.id())?;
        if !referenced.contains(&(
            variant.role_id().as_str().to_owned(),
            id.as_str().to_owned(),
        )) {
            continue;
        }
        let order = role_orders
            .entry(variant.role_id().as_str().to_owned())
            .and_modify(|value| *value += 1)
            .or_insert(1);
        variants.push(AutoloopVariant::try_new(
            variant.role_id().clone(),
            id,
            variant.display_name().to_owned(),
            *order,
            variant.is_archived(),
        )?);
    }
    let revision = existing
        .revision()
        .checked_add(1)
        .ok_or(AutoloopDefaultsError::Overflow)?;
    let migrated = AutoloopCatalog::try_new(
        revision,
        AUTOLOOP_CATALOG_DEFAULTS_VERSION,
        existing.themes().to_vec(),
        variants,
        cells,
    )?;
    migrated.validate_roles(phrase_roles)?;
    Ok(migrated)
}

fn uses_row_major_sound_switch_surface(catalog: &AutoloopCatalog) -> bool {
    catalog
        .cells()
        .iter()
        .filter_map(|cell| mapping_slot_number(cell.variant_id()))
        .any(|number| (9..=32).contains(&number))
}

fn is_generated_sound_switch_default(cell: &AutoloopMatrixCell, catalog: &AutoloopCatalog) -> bool {
    let Some(number) = mapping_slot_number(cell.variant_id()) else {
        return false;
    };
    let trailing_number = cell
        .display_name()
        .split_whitespace()
        .next_back()
        .and_then(|value| value.parse::<u16>().ok());
    let role_name = cell.role_id().as_str().replace('-', " ").to_uppercase();
    if trailing_number == Some(number)
        && cell.display_name().starts_with(&format!("{role_name} · "))
    {
        return true;
    }
    catalog
        .themes()
        .iter()
        .find(|theme| theme.id() == cell.theme_id())
        .is_some_and(|theme| {
            trailing_number.is_some_and(|value| (1..=32).contains(&value))
                && cell
                    .display_name()
                    .starts_with(&format!("{} · ", theme.display_name()))
        })
}

fn transposed_mapping_id(id: &VariantId) -> Result<VariantId, AutoloopDefaultsError> {
    let Some(number) = mapping_slot_number(id).filter(|number| (1..=32).contains(number)) else {
        return Ok(id.clone());
    };
    let zero_based = usize::from(number - 1);
    let row = zero_based / 4;
    let column = zero_based % 4;
    let transposed = column
        .checked_mul(8)
        .and_then(|value| value.checked_add(row))
        .and_then(|value| value.checked_add(1))
        .ok_or(AutoloopDefaultsError::Overflow)?;
    Ok(VariantId::try_new(format!("mapping-{transposed}"))?)
}

fn mapping_slot_number(id: &VariantId) -> Option<u16> {
    id.as_str().strip_prefix("mapping-")?.parse().ok()
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
        assert_eq!(upgraded.defaults_version(), 4);
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

    #[test]
    fn version_three_grid_positions_move_to_sound_switch_column_major_slots()
    -> Result<(), Box<dyn std::error::Error>> {
        let empty_roles = PhraseRoleCatalog::try_new(0, 0, Vec::new(), Vec::new())?;
        let phrase_roles = seeded_phrase_role_catalog(&empty_roles)?;
        let empty_catalog = AutoloopCatalog::try_new(0, 0, Vec::new(), Vec::new(), Vec::new())?;
        let seeded = seeded_autoloop_catalog(&empty_catalog, &phrase_roles)?;
        let customized = seeded
            .rename_theme(ThemeId::new(1), "Blue - Pink")?
            .set_mapping(
                ThemeId::new(1),
                VariantId::try_new("mapping-2")?,
                PhraseRoleId::try_new("breakdown-1")?,
                Some("BD BRIDGE BLUE PINK".to_owned()),
            )?
            .set_mapping(
                ThemeId::new(1),
                VariantId::try_new("mapping-3")?,
                PhraseRoleId::try_new("buildup-1")?,
                Some("BUILDUP BLUE PINK 1A".to_owned()),
            )?
            .set_mapping(
                ThemeId::new(1),
                VariantId::try_new("mapping-4")?,
                PhraseRoleId::try_new("bridge")?,
                Some("BRIDGE BLUE PINK".to_owned()),
            )?
            // A historic v1-to-v2 seed could leave the previous visual index
            // in the generated name. It is still a default, not user data.
            .set_mapping(
                ThemeId::new(4),
                VariantId::try_new("mapping-2")?,
                PhraseRoleId::try_new("breakdown-1")?,
                Some("Ultraviolet · Breakdown 1 3".to_owned()),
            )?;
        // Production catalogs may still report version 1 after having been
        // expanded and edited through the 32-slot profile UI.
        let legacy_32_slot_catalog = AutoloopCatalog::try_new(
            customized.revision(),
            1,
            customized.themes().to_vec(),
            customized.variants().to_vec(),
            customized.cells().to_vec(),
        )?;

        let migrated = seeded_autoloop_catalog(&legacy_32_slot_catalog, &phrase_roles)?;

        assert_eq!(migrated.defaults_version(), 4);
        assert_eq!(migrated.revision(), legacy_32_slot_catalog.revision() + 1);
        assert_eq!(migrated.themes()[0].display_name(), "Blue - Pink");
        let bridge = migrated
            .cells()
            .iter()
            .find(|cell| {
                cell.theme_id() == ThemeId::new(1) && cell.variant_id().as_str() == "mapping-9"
            })
            .ok_or("transposed mapping missing")?;
        assert_eq!(bridge.role_id().as_str(), "breakdown-1");
        assert_eq!(bridge.display_name(), "BD BRIDGE BLUE PINK");
        assert!(migrated.cells().iter().any(|cell| {
            cell.theme_id() == ThemeId::new(1)
                && cell.variant_id().as_str() == "mapping-17"
                && cell.display_name() == "BUILDUP BLUE PINK 1A"
        }));
        assert!(migrated.cells().iter().any(|cell| {
            cell.theme_id() == ThemeId::new(1)
                && cell.variant_id().as_str() == "mapping-25"
                && cell.display_name() == "BRIDGE BLUE PINK"
        }));
        assert_eq!(migrated.cells().len(), 3);
        Ok(())
    }

    #[test]
    fn original_eight_button_catalog_uses_the_regular_upgrade_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let empty_roles = PhraseRoleCatalog::try_new(0, 0, Vec::new(), Vec::new())?;
        let phrase_roles = seeded_phrase_role_catalog(&empty_roles)?;
        let empty_catalog = AutoloopCatalog::try_new(0, 0, Vec::new(), Vec::new(), Vec::new())?;
        let seeded = seeded_autoloop_catalog(&empty_catalog, &phrase_roles)?;
        let eight_button_cells = seeded
            .cells()
            .iter()
            .filter(|cell| mapping_slot_number(cell.variant_id()).is_some_and(|slot| slot <= 8))
            .cloned()
            .collect::<Vec<_>>();
        let eight_button_variants = seeded
            .variants()
            .iter()
            .filter(|variant| mapping_slot_number(variant.id()).is_some_and(|slot| slot <= 8))
            .cloned()
            .collect::<Vec<_>>();
        let legacy = AutoloopCatalog::try_new(
            9,
            1,
            seeded.themes().to_vec(),
            eight_button_variants,
            eight_button_cells,
        )?;

        let upgraded = seeded_autoloop_catalog(&legacy, &phrase_roles)?;

        assert_eq!(upgraded.defaults_version(), 4);
        assert_eq!(upgraded.cells().len(), 128);
        assert!(upgraded.cells().iter().any(|cell| {
            cell.theme_id() == ThemeId::new(1) && cell.variant_id().as_str() == "mapping-2"
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
