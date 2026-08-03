use lumi_domain::ThemeId;
use lumi_library::{
    AUTOLOOP_CATALOG_DEFAULTS_VERSION, AutoloopCatalog, AutoloopCatalogError, AutoloopEntryId,
    AutoloopMatrixCell, AutoloopResolutionReason, AutoloopTheme, AutoloopVariant,
    AutoloopVariantMove, PhraseLoopStrategy, PhraseRoleId, ThemeSpecificVariant, VariantId,
};

#[test]
fn four_themes_are_columns_and_variants_are_flexible_role_scoped_rows()
-> Result<(), Box<dyn std::error::Error>> {
    let catalog = fixture()?;

    assert_eq!(catalog.themes().len(), 4);
    assert_eq!(
        catalog
            .variants()
            .iter()
            .filter(|variant| variant.role_id().as_str() == "breakdown-1")
            .count(),
        2
    );
    assert_eq!(
        catalog
            .variants()
            .iter()
            .filter(|variant| variant.role_id().as_str() == "synth")
            .count(),
        1
    );
    assert!(catalog.cells().iter().all(|cell| {
        !cell.entry_id().as_str().contains("midi")
            && !cell.entry_id().as_str().contains("slot")
            && !cell.entry_id().as_str().contains("bank")
    }));
    Ok(())
}

#[test]
fn the_same_row_resolves_to_distinct_entries_for_each_theme()
-> Result<(), Box<dyn std::error::Error>> {
    let catalog = fixture()?;
    let role = PhraseRoleId::try_new("synth")?;
    let variant = VariantId::try_new("variant-1")?;
    let entries = (1..=4)
        .map(|theme| {
            catalog
                .resolve(ThemeId::new(theme), &role, Some(&variant), 1)
                .map(|resolution| {
                    assert_eq!(resolution.role_id(), &role);
                    assert_eq!(resolution.variant_id(), &variant);
                    assert_eq!(resolution.reason(), &AutoloopResolutionReason::ExactVariant);
                    resolution.entry_id().as_str().to_owned()
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(entries.len(), 4);
    assert_eq!(
        entries
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    Ok(())
}

#[test]
fn missing_cells_are_preflight_evidence_and_fallback_never_changes_role()
-> Result<(), Box<dyn std::error::Error>> {
    let catalog = fixture()?;
    let breakdown = PhraseRoleId::try_new("breakdown-1")?;
    let missing_variant = VariantId::try_new("variant-2")?;
    let missing = catalog.missing_cells();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].theme_id(), ThemeId::new(4));
    assert_eq!(missing[0].role_id(), &breakdown);
    assert_eq!(missing[0].variant_id(), &missing_variant);

    let fallback = catalog.resolve(ThemeId::new(4), &breakdown, Some(&missing_variant), 1)?;
    assert_eq!(fallback.role_id(), &breakdown);
    assert_eq!(fallback.variant_id().as_str(), "variant-1");
    assert!(matches!(
        fallback.reason(),
        AutoloopResolutionReason::SameRoleFallback { requested_variant_id }
            if requested_variant_id == &missing_variant
    ));

    let unknown_role = PhraseRoleId::try_new("drop")?;
    assert_eq!(
        catalog.resolve(ThemeId::new(4), &unknown_role, None, 1),
        Err(AutoloopCatalogError::MissingRoleCoverage)
    );
    Ok(())
}

#[test]
fn synth_never_silently_resolves_to_a_different_role() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = fixture()?.set_cell(
        ThemeId::new(2),
        &PhraseRoleId::try_new("synth")?,
        &VariantId::try_new("variant-1")?,
        None,
    )?;

    assert_eq!(
        catalog.resolve(
            ThemeId::new(2),
            &PhraseRoleId::try_new("synth")?,
            None,
            catalog.revision(),
        ),
        Err(AutoloopCatalogError::MissingRoleCoverage)
    );
    Ok(())
}

#[test]
fn variant_mutations_preserve_stable_row_identity_and_revision_safety()
-> Result<(), Box<dyn std::error::Error>> {
    let role = PhraseRoleId::try_new("synth")?;
    let initial = fixture()?;
    let added = initial.add_variant(role.clone(), "Spark")?;
    let added_variant = added
        .variants()
        .iter()
        .find(|variant| variant.role_id() == &role && variant.display_name() == "Spark")
        .ok_or("new variant is missing")?;
    let stable_id = added_variant.id().clone();
    let renamed = added.rename_variant(&role, &stable_id, "Laser Spark")?;
    let moved = renamed.move_variant(&role, &stable_id, AutoloopVariantMove::Earlier)?;
    let archived = moved.set_variant_archived(&role, &stable_id, true)?;
    let restored = archived.set_variant_archived(&role, &stable_id, false)?;
    assert_eq!(
        restored
            .variants()
            .iter()
            .find(|variant| variant.role_id() == &role && variant.id() == &stable_id)
            .map(AutoloopVariant::display_name),
        Some("Laser Spark")
    );
    assert!(matches!(
        restored.resolve(ThemeId::new(1), &role, None, 1),
        Err(AutoloopCatalogError::RevisionConflict { .. })
    ));
    Ok(())
}

#[test]
fn duplicate_cells_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = fixture()?;
    let mut cells = catalog.cells().to_vec();
    cells.push(cells[0].clone());
    assert_eq!(
        AutoloopCatalog::try_new(
            catalog.revision(),
            catalog.defaults_version(),
            catalog.themes().to_vec(),
            catalog.variants().to_vec(),
            cells,
        ),
        Err(AutoloopCatalogError::DuplicateCell)
    );
    Ok(())
}

#[test]
fn persisted_catalogs_require_exactly_four_theme_targets() -> Result<(), Box<dyn std::error::Error>>
{
    let catalog = fixture()?;
    assert_eq!(
        AutoloopCatalog::try_new(
            catalog.revision(),
            catalog.defaults_version(),
            catalog.themes()[..3].to_vec(),
            catalog.variants().to_vec(),
            catalog.cells().to_vec(),
        ),
        Err(AutoloopCatalogError::InvalidThemeCount)
    );
    Ok(())
}

#[test]
fn loop_templates_resolve_after_theme_selection_without_storing_a_theme()
-> Result<(), Box<dyn std::error::Error>> {
    let catalog = fixture()?;
    let role = PhraseRoleId::try_new("breakdown-1")?;
    let automatic = PhraseLoopStrategy::Auto;
    let automatic_entries = (1..=4)
        .map(|theme| {
            catalog
                .resolve_loop_strategy(ThemeId::new(theme), &role, &automatic, catalog.revision())
                .map(|resolution| {
                    assert_eq!(resolution.reason(), &AutoloopResolutionReason::Automatic);
                    resolution.entry_id().as_str().to_owned()
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        automatic_entries
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    let fixed = PhraseLoopStrategy::FixedVariant(VariantId::try_new("variant-1")?);
    let fixed_entries = (1..=4)
        .map(|theme| {
            catalog
                .resolve_loop_strategy(ThemeId::new(theme), &role, &fixed, catalog.revision())
                .map(|resolution| {
                    assert_eq!(resolution.variant_id().as_str(), "variant-1");
                    resolution.entry_id().as_str().to_owned()
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        fixed_entries
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );

    let exact = PhraseLoopStrategy::ThemeSpecificExact(vec![ThemeSpecificVariant::new(
        ThemeId::new(2),
        VariantId::try_new("variant-2")?,
    )]);
    let theme_one = catalog.resolve_loop_strategy(ThemeId::new(1), &role, &exact, 1)?;
    let theme_two = catalog.resolve_loop_strategy(ThemeId::new(2), &role, &exact, 1)?;
    assert_eq!(theme_one.reason(), &AutoloopResolutionReason::Automatic);
    assert_eq!(
        theme_two.reason(),
        &AutoloopResolutionReason::ThemeSpecificExact
    );
    assert_eq!(theme_two.variant_id().as_str(), "variant-2");
    Ok(())
}

#[test]
fn fixed_and_theme_exact_strategies_fail_closed_when_their_row_is_incompatible()
-> Result<(), Box<dyn std::error::Error>> {
    let catalog = fixture()?;
    let synth = PhraseRoleId::try_new("synth")?;
    let breakdown_variant = VariantId::try_new("variant-2")?;
    assert_eq!(
        catalog.validate_loop_strategy(
            &synth,
            &PhraseLoopStrategy::FixedVariant(breakdown_variant.clone()),
        ),
        Err(AutoloopCatalogError::IncompatibleVariant)
    );

    let missing_exact = PhraseLoopStrategy::ThemeSpecificExact(vec![ThemeSpecificVariant::new(
        ThemeId::new(4),
        breakdown_variant,
    )]);
    assert_eq!(
        catalog.validate_loop_strategy(&PhraseRoleId::try_new("breakdown-1")?, &missing_exact),
        Err(AutoloopCatalogError::MissingExactCell)
    );
    Ok(())
}

fn fixture() -> Result<AutoloopCatalog, Box<dyn std::error::Error>> {
    let themes = ["Electric Bloom", "Deep Ocean", "Solar Flare", "Ultraviolet"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            Ok(AutoloopTheme::try_new(
                ThemeId::new(u64::try_from(index + 1)?),
                name,
                u16::try_from(index + 1)?,
            )?)
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let variants = vec![
        variant("breakdown-1", "variant-1", "Pulse", 1)?,
        variant("breakdown-1", "variant-2", "Wave", 2)?,
        variant("synth", "variant-1", "Saw", 1)?,
    ];
    let mut cells = Vec::new();
    for theme in 1..=4 {
        cells.push(cell(theme, "breakdown-1", "variant-1", "Pulse")?);
        cells.push(cell(theme, "synth", "variant-1", "Saw")?);
        if theme != 4 {
            cells.push(cell(theme, "breakdown-1", "variant-2", "Wave")?);
        }
    }
    Ok(AutoloopCatalog::try_new(
        1,
        AUTOLOOP_CATALOG_DEFAULTS_VERSION,
        themes,
        variants,
        cells,
    )?)
}

fn variant(
    role: &str,
    id: &str,
    name: &str,
    order: u16,
) -> Result<AutoloopVariant, Box<dyn std::error::Error>> {
    Ok(AutoloopVariant::try_new(
        PhraseRoleId::try_new(role)?,
        VariantId::try_new(id)?,
        name,
        order,
        false,
    )?)
}

fn cell(
    theme: u64,
    role: &str,
    variant: &str,
    name: &str,
) -> Result<AutoloopMatrixCell, Box<dyn std::error::Error>> {
    Ok(AutoloopMatrixCell::try_new(
        ThemeId::new(theme),
        PhraseRoleId::try_new(role)?,
        VariantId::try_new(variant)?,
        AutoloopEntryId::try_new(format!("theme-{theme}--{role}--{variant}"))?,
        format!("Theme {theme} {name}"),
    )?)
}
