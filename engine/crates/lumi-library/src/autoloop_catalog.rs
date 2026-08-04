use std::collections::{HashMap, HashSet};

use lumi_domain::ThemeId;

use crate::{AutoloopEntryId, PhraseLoopStrategy, PhraseRoleCatalog, PhraseRoleId, VariantId};

pub const AUTOLOOP_CATALOG_DEFAULTS_VERSION: u16 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoloopTheme {
    id: ThemeId,
    display_name: String,
    sort_order: u16,
}

impl AutoloopTheme {
    pub fn try_new(
        id: ThemeId,
        display_name: impl Into<String>,
        sort_order: u16,
    ) -> Result<Self, AutoloopCatalogError> {
        if id.value() == 0 {
            return Err(AutoloopCatalogError::InvalidThemeId);
        }
        if sort_order == 0 {
            return Err(AutoloopCatalogError::InvalidSortOrder);
        }
        Ok(Self {
            id,
            display_name: validated_name(display_name.into())?,
            sort_order,
        })
    }

    #[must_use]
    pub const fn id(&self) -> ThemeId {
        self.id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn sort_order(&self) -> u16 {
        self.sort_order
    }

    fn renamed(&self, display_name: String) -> Result<Self, AutoloopCatalogError> {
        Self::try_new(self.id, display_name, self.sort_order)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoloopVariant {
    role_id: PhraseRoleId,
    id: VariantId,
    display_name: String,
    sort_order: u16,
    archived: bool,
}

impl AutoloopVariant {
    pub fn try_new(
        role_id: PhraseRoleId,
        id: VariantId,
        display_name: impl Into<String>,
        sort_order: u16,
        archived: bool,
    ) -> Result<Self, AutoloopCatalogError> {
        if sort_order == 0 {
            return Err(AutoloopCatalogError::InvalidSortOrder);
        }
        Ok(Self {
            role_id,
            id,
            display_name: validated_name(display_name.into())?,
            sort_order,
            archived,
        })
    }

    #[must_use]
    pub const fn role_id(&self) -> &PhraseRoleId {
        &self.role_id
    }

    #[must_use]
    pub const fn id(&self) -> &VariantId {
        &self.id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn sort_order(&self) -> u16 {
        self.sort_order
    }

    #[must_use]
    pub const fn is_archived(&self) -> bool {
        self.archived
    }

    fn rebuilt(
        &self,
        display_name: String,
        sort_order: u16,
        archived: bool,
    ) -> Result<Self, AutoloopCatalogError> {
        Self::try_new(
            self.role_id.clone(),
            self.id.clone(),
            display_name,
            sort_order,
            archived,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoloopMatrixCell {
    theme_id: ThemeId,
    role_id: PhraseRoleId,
    variant_id: VariantId,
    entry_id: AutoloopEntryId,
    display_name: String,
}

impl AutoloopMatrixCell {
    pub fn try_new(
        theme_id: ThemeId,
        role_id: PhraseRoleId,
        variant_id: VariantId,
        entry_id: AutoloopEntryId,
        display_name: impl Into<String>,
    ) -> Result<Self, AutoloopCatalogError> {
        if theme_id.value() == 0 {
            return Err(AutoloopCatalogError::InvalidThemeId);
        }
        Ok(Self {
            theme_id,
            role_id,
            variant_id,
            entry_id,
            display_name: validated_name(display_name.into())?,
        })
    }

    #[must_use]
    pub const fn theme_id(&self) -> ThemeId {
        self.theme_id
    }

    #[must_use]
    pub const fn role_id(&self) -> &PhraseRoleId {
        &self.role_id
    }

    #[must_use]
    pub const fn variant_id(&self) -> &VariantId {
        &self.variant_id
    }

    #[must_use]
    pub const fn entry_id(&self) -> &AutoloopEntryId {
        &self.entry_id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutoloopVariantMove {
    Earlier,
    Later,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingAutoloopCell {
    theme_id: ThemeId,
    role_id: PhraseRoleId,
    variant_id: VariantId,
}

impl MissingAutoloopCell {
    #[must_use]
    pub const fn theme_id(&self) -> ThemeId {
        self.theme_id
    }

    #[must_use]
    pub const fn role_id(&self) -> &PhraseRoleId {
        &self.role_id
    }

    #[must_use]
    pub const fn variant_id(&self) -> &VariantId {
        &self.variant_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AutoloopResolutionReason {
    Automatic,
    ExactVariant,
    ThemeSpecificExact,
    SameRoleFallback { requested_variant_id: VariantId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoloopResolution {
    theme_id: ThemeId,
    role_id: PhraseRoleId,
    variant_id: VariantId,
    entry_id: AutoloopEntryId,
    display_name: String,
    catalog_revision: u64,
    reason: AutoloopResolutionReason,
}

impl AutoloopResolution {
    #[must_use]
    pub const fn theme_id(&self) -> ThemeId {
        self.theme_id
    }

    #[must_use]
    pub const fn role_id(&self) -> &PhraseRoleId {
        &self.role_id
    }

    #[must_use]
    pub const fn variant_id(&self) -> &VariantId {
        &self.variant_id
    }

    #[must_use]
    pub const fn entry_id(&self) -> &AutoloopEntryId {
        &self.entry_id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn catalog_revision(&self) -> u64 {
        self.catalog_revision
    }

    #[must_use]
    pub const fn reason(&self) -> &AutoloopResolutionReason {
        &self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoloopCatalog {
    revision: u64,
    defaults_version: u16,
    themes: Vec<AutoloopTheme>,
    variants: Vec<AutoloopVariant>,
    cells: Vec<AutoloopMatrixCell>,
}

impl AutoloopCatalog {
    pub fn try_new(
        revision: u64,
        defaults_version: u16,
        themes: Vec<AutoloopTheme>,
        variants: Vec<AutoloopVariant>,
        cells: Vec<AutoloopMatrixCell>,
    ) -> Result<Self, AutoloopCatalogError> {
        if revision == 0 && (defaults_version != 0 || !themes.is_empty() || !variants.is_empty()) {
            return Err(AutoloopCatalogError::InvalidRevision);
        }
        if revision > 0 && themes.len() != 4 {
            return Err(AutoloopCatalogError::InvalidThemeCount);
        }
        validate_themes(&themes)?;
        validate_variants(&variants)?;

        let theme_ids = themes.iter().map(AutoloopTheme::id).collect::<HashSet<_>>();
        let row_keys = variants
            .iter()
            .map(|variant| {
                (
                    variant.role_id().as_str().to_owned(),
                    variant.id().as_str().to_owned(),
                )
            })
            .collect::<HashSet<_>>();
        let mut cell_keys = HashSet::new();
        let mut entry_ids = HashSet::new();
        for cell in &cells {
            if !theme_ids.contains(&cell.theme_id()) {
                return Err(AutoloopCatalogError::UnknownTheme);
            }
            if !row_keys.contains(&(
                cell.role_id().as_str().to_owned(),
                cell.variant_id().as_str().to_owned(),
            )) {
                return Err(AutoloopCatalogError::UnknownVariant);
            }
            if !cell_keys.insert((
                cell.theme_id().value(),
                cell.role_id().as_str().to_owned(),
                cell.variant_id().as_str().to_owned(),
            )) {
                return Err(AutoloopCatalogError::DuplicateCell);
            }
            if !entry_ids.insert(cell.entry_id().as_str().to_owned()) {
                return Err(AutoloopCatalogError::DuplicateEntryId);
            }
        }
        let mut mapping_keys = HashSet::new();
        for cell in &cells {
            if catalog_mapping_number(cell.variant_id()).is_some()
                && !mapping_keys.insert((
                    cell.theme_id().value(),
                    cell.variant_id().as_str().to_owned(),
                ))
            {
                return Err(AutoloopCatalogError::DuplicateMappingIdentity);
            }
        }
        Ok(Self {
            revision,
            defaults_version,
            themes,
            variants,
            cells,
        })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn defaults_version(&self) -> u16 {
        self.defaults_version
    }

    #[must_use]
    pub fn themes(&self) -> &[AutoloopTheme] {
        &self.themes
    }

    #[must_use]
    pub fn variants(&self) -> &[AutoloopVariant] {
        &self.variants
    }

    #[must_use]
    pub fn cells(&self) -> &[AutoloopMatrixCell] {
        &self.cells
    }

    pub fn validate_roles(
        &self,
        phrase_roles: &PhraseRoleCatalog,
    ) -> Result<(), AutoloopCatalogError> {
        let known = phrase_roles
            .roles()
            .iter()
            .map(|role| role.id().as_str())
            .collect::<HashSet<_>>();
        if self
            .variants
            .iter()
            .any(|variant| !known.contains(variant.role_id().as_str()))
        {
            return Err(AutoloopCatalogError::UnknownPhraseRole);
        }
        Ok(())
    }

    #[must_use]
    pub fn missing_cells(&self) -> Vec<MissingAutoloopCell> {
        let populated = self
            .cells
            .iter()
            .map(|cell| {
                (
                    cell.theme_id().value(),
                    cell.role_id().as_str(),
                    cell.variant_id().as_str(),
                )
            })
            .collect::<HashSet<_>>();
        let mut missing = Vec::new();
        for theme in &self.themes {
            for variant in self.variants.iter().filter(|variant| {
                !variant.is_archived() && catalog_mapping_number(variant.id()).is_none()
            }) {
                if !populated.contains(&(
                    theme.id().value(),
                    variant.role_id().as_str(),
                    variant.id().as_str(),
                )) {
                    missing.push(MissingAutoloopCell {
                        theme_id: theme.id(),
                        role_id: variant.role_id().clone(),
                        variant_id: variant.id().clone(),
                    });
                }
            }
        }
        missing
    }

    pub fn resolve(
        &self,
        theme_id: ThemeId,
        role_id: &PhraseRoleId,
        preferred_variant_id: Option<&VariantId>,
        expected_revision: u64,
    ) -> Result<AutoloopResolution, AutoloopCatalogError> {
        if expected_revision != self.revision {
            return Err(AutoloopCatalogError::RevisionConflict {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        if !self.themes.iter().any(|theme| theme.id() == theme_id) {
            return Err(AutoloopCatalogError::UnknownTheme);
        }
        let active_variants = self
            .variants
            .iter()
            .filter(|variant| variant.role_id() == role_id && !variant.is_archived())
            .collect::<Vec<_>>();
        if active_variants.is_empty() {
            return Err(AutoloopCatalogError::MissingRoleCoverage);
        }
        if let Some(preferred) = preferred_variant_id
            && let Some(variant) = active_variants
                .iter()
                .find(|variant| variant.id() == preferred)
            && let Some(cell) = self.cell(theme_id, role_id, variant.id())
        {
            return Ok(self.resolution(cell, AutoloopResolutionReason::ExactVariant));
        }
        let fallback = active_variants
            .iter()
            .find_map(|variant| self.cell(theme_id, role_id, variant.id()))
            .ok_or(AutoloopCatalogError::MissingRoleCoverage)?;
        let reason =
            preferred_variant_id.map_or(AutoloopResolutionReason::Automatic, |preferred| {
                AutoloopResolutionReason::SameRoleFallback {
                    requested_variant_id: preferred.clone(),
                }
            });
        Ok(self.resolution(fallback, reason))
    }

    pub fn validate_loop_strategy(
        &self,
        role_id: &PhraseRoleId,
        strategy: &PhraseLoopStrategy,
    ) -> Result<(), AutoloopCatalogError> {
        match strategy {
            PhraseLoopStrategy::Auto => {}
            PhraseLoopStrategy::FixedVariant(variant_id) => {
                self.require_active_role_variant(role_id, variant_id)?;
            }
            PhraseLoopStrategy::ThemeSpecificExact(overrides) => {
                for override_value in overrides {
                    if !self
                        .themes
                        .iter()
                        .any(|theme| theme.id() == override_value.theme_id())
                    {
                        return Err(AutoloopCatalogError::UnknownTheme);
                    }
                    self.require_active_role_variant(role_id, override_value.variant_id())?;
                    if self
                        .cell(
                            override_value.theme_id(),
                            role_id,
                            override_value.variant_id(),
                        )
                        .is_none()
                    {
                        return Err(AutoloopCatalogError::MissingExactCell);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn resolve_loop_strategy(
        &self,
        theme_id: ThemeId,
        role_id: &PhraseRoleId,
        strategy: &PhraseLoopStrategy,
        expected_revision: u64,
    ) -> Result<AutoloopResolution, AutoloopCatalogError> {
        if expected_revision != self.revision {
            return Err(AutoloopCatalogError::RevisionConflict {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        if !self.themes.iter().any(|theme| theme.id() == theme_id) {
            return Err(AutoloopCatalogError::UnknownTheme);
        }
        match strategy {
            PhraseLoopStrategy::Auto => self.resolve(theme_id, role_id, None, expected_revision),
            PhraseLoopStrategy::FixedVariant(variant_id) => self.resolve_exact_strategy(
                theme_id,
                role_id,
                variant_id,
                AutoloopResolutionReason::ExactVariant,
            ),
            PhraseLoopStrategy::ThemeSpecificExact(overrides) => {
                let Some(override_value) =
                    overrides.iter().find(|value| value.theme_id() == theme_id)
                else {
                    return self.resolve(theme_id, role_id, None, expected_revision);
                };
                self.resolve_exact_strategy(
                    theme_id,
                    role_id,
                    override_value.variant_id(),
                    AutoloopResolutionReason::ThemeSpecificExact,
                )
            }
        }
    }

    pub fn rename_theme(
        &self,
        theme_id: ThemeId,
        display_name: impl Into<String>,
    ) -> Result<Self, AutoloopCatalogError> {
        let display_name = validated_name(display_name.into())?;
        if self.themes.iter().any(|theme| {
            theme.id() != theme_id && theme.display_name().eq_ignore_ascii_case(&display_name)
        }) {
            return Err(AutoloopCatalogError::DuplicateDisplayName);
        }
        let index = self
            .themes
            .iter()
            .position(|theme| theme.id() == theme_id)
            .ok_or(AutoloopCatalogError::UnknownTheme)?;
        if self.themes[index].display_name() == display_name {
            return Err(AutoloopCatalogError::NoChange);
        }
        let mut themes = self.themes.clone();
        themes[index] = themes[index].renamed(display_name)?;
        self.revised(themes, self.variants.clone(), self.cells.clone())
    }

    pub fn add_variant(
        &self,
        role_id: PhraseRoleId,
        display_name: impl Into<String>,
    ) -> Result<Self, AutoloopCatalogError> {
        let display_name = validated_name(display_name.into())?;
        self.ensure_unique_variant_name(&role_id, &display_name, None)?;
        let role_variants = self
            .variants
            .iter()
            .filter(|variant| variant.role_id() == &role_id)
            .collect::<Vec<_>>();
        let mut suffix = 1_u64;
        let id = loop {
            let candidate = VariantId::try_new(format!("variant-{suffix}"))
                .map_err(|_| AutoloopCatalogError::IdentifierOverflow)?;
            if !role_variants
                .iter()
                .any(|variant| variant.id() == &candidate)
            {
                break candidate;
            }
            suffix = suffix
                .checked_add(1)
                .ok_or(AutoloopCatalogError::ArithmeticOverflow)?;
        };
        let sort_order = u16::try_from(role_variants.len() + 1)
            .map_err(|_| AutoloopCatalogError::TooManyVariants)?;
        let mut variants = self.variants.clone();
        variants.push(AutoloopVariant::try_new(
            role_id,
            id,
            display_name,
            sort_order,
            false,
        )?);
        sort_variants(&mut variants);
        self.revised(self.themes.clone(), variants, self.cells.clone())
    }

    pub fn rename_variant(
        &self,
        role_id: &PhraseRoleId,
        variant_id: &VariantId,
        display_name: impl Into<String>,
    ) -> Result<Self, AutoloopCatalogError> {
        let display_name = validated_name(display_name.into())?;
        self.ensure_unique_variant_name(role_id, &display_name, Some(variant_id))?;
        let index = self.variant_index(role_id, variant_id)?;
        if self.variants[index].display_name() == display_name {
            return Err(AutoloopCatalogError::NoChange);
        }
        let mut variants = self.variants.clone();
        variants[index] = variants[index].rebuilt(
            display_name,
            variants[index].sort_order(),
            variants[index].is_archived(),
        )?;
        self.revised(self.themes.clone(), variants, self.cells.clone())
    }

    pub fn move_variant(
        &self,
        role_id: &PhraseRoleId,
        variant_id: &VariantId,
        direction: AutoloopVariantMove,
    ) -> Result<Self, AutoloopCatalogError> {
        let current_index = self.variant_index(role_id, variant_id)?;
        let current_order = self.variants[current_index].sort_order();
        let target_order = match direction {
            AutoloopVariantMove::Earlier => current_order.checked_sub(1),
            AutoloopVariantMove::Later => current_order.checked_add(1),
        }
        .ok_or(AutoloopCatalogError::NoChange)?;
        let target_index = self
            .variants
            .iter()
            .position(|variant| {
                variant.role_id() == role_id && variant.sort_order() == target_order
            })
            .ok_or(AutoloopCatalogError::NoChange)?;
        let mut variants = self.variants.clone();
        let current = variants[current_index].clone();
        let target = variants[target_index].clone();
        variants[current_index] = current.rebuilt(
            current.display_name().to_owned(),
            target_order,
            current.is_archived(),
        )?;
        variants[target_index] = target.rebuilt(
            target.display_name().to_owned(),
            current_order,
            target.is_archived(),
        )?;
        sort_variants(&mut variants);
        self.revised(self.themes.clone(), variants, self.cells.clone())
    }

    pub fn set_variant_archived(
        &self,
        role_id: &PhraseRoleId,
        variant_id: &VariantId,
        archived: bool,
    ) -> Result<Self, AutoloopCatalogError> {
        let index = self.variant_index(role_id, variant_id)?;
        if self.variants[index].is_archived() == archived {
            return Err(AutoloopCatalogError::NoChange);
        }
        if archived
            && self
                .variants
                .iter()
                .filter(|variant| variant.role_id() == role_id && !variant.is_archived())
                .count()
                == 1
        {
            return Err(AutoloopCatalogError::NoActiveVariantsForRole);
        }
        let mut variants = self.variants.clone();
        variants[index] = variants[index].rebuilt(
            variants[index].display_name().to_owned(),
            variants[index].sort_order(),
            archived,
        )?;
        self.revised(self.themes.clone(), variants, self.cells.clone())
    }

    pub fn set_cell(
        &self,
        theme_id: ThemeId,
        role_id: &PhraseRoleId,
        variant_id: &VariantId,
        display_name: Option<String>,
    ) -> Result<Self, AutoloopCatalogError> {
        if !self.themes.iter().any(|theme| theme.id() == theme_id) {
            return Err(AutoloopCatalogError::UnknownTheme);
        }
        self.variant_index(role_id, variant_id)?;
        let mut cells = self.cells.clone();
        let existing = cells.iter().position(|cell| {
            cell.theme_id() == theme_id
                && cell.role_id() == role_id
                && cell.variant_id() == variant_id
        });
        match (existing, display_name) {
            (Some(index), Some(display_name)) => {
                let display_name = validated_name(display_name)?;
                if cells[index].display_name() == display_name {
                    return Err(AutoloopCatalogError::NoChange);
                }
                cells[index] = AutoloopMatrixCell::try_new(
                    theme_id,
                    role_id.clone(),
                    variant_id.clone(),
                    cells[index].entry_id().clone(),
                    display_name,
                )?;
            }
            (None, Some(display_name)) => {
                cells.push(AutoloopMatrixCell::try_new(
                    theme_id,
                    role_id.clone(),
                    variant_id.clone(),
                    generated_entry_id(theme_id, role_id, variant_id)?,
                    display_name,
                )?);
            }
            (Some(index), None) => {
                cells.remove(index);
            }
            (None, None) => return Err(AutoloopCatalogError::NoChange),
        }
        sort_cells(&mut cells, &self.themes, &self.variants);
        self.revised(self.themes.clone(), self.variants.clone(), cells)
    }

    pub fn set_mapping(
        &self,
        theme_id: ThemeId,
        mapping_id: VariantId,
        role_id: PhraseRoleId,
        display_name: Option<String>,
    ) -> Result<Self, AutoloopCatalogError> {
        if !self.themes.iter().any(|theme| theme.id() == theme_id) {
            return Err(AutoloopCatalogError::UnknownTheme);
        }
        let mut cells = self.cells.clone();
        let existing = cells
            .iter()
            .position(|cell| cell.theme_id() == theme_id && cell.variant_id() == &mapping_id);
        let existing_cell = existing.map(|index| cells.remove(index));
        let Some(display_name) = display_name else {
            if existing_cell.is_none() {
                return Err(AutoloopCatalogError::NoChange);
            }
            let variants = prune_unused_variants(self.variants.clone(), &cells)?;
            sort_cells(&mut cells, &self.themes, &variants);
            return self.revised(self.themes.clone(), variants, cells);
        };
        let display_name = validated_name(display_name)?;
        if let Some(existing_cell) = &existing_cell
            && existing_cell.role_id() == &role_id
            && existing_cell.display_name() == display_name
        {
            return Err(AutoloopCatalogError::NoChange);
        }
        let mut variants = self.variants.clone();
        if !variants
            .iter()
            .any(|variant| variant.role_id() == &role_id && variant.id() == &mapping_id)
        {
            let sort_order = u16::try_from(
                variants
                    .iter()
                    .filter(|variant| variant.role_id() == &role_id)
                    .count()
                    + 1,
            )
            .map_err(|_| AutoloopCatalogError::TooManyVariants)?;
            variants.push(AutoloopVariant::try_new(
                role_id.clone(),
                mapping_id.clone(),
                format!("Output {}", mapping_id.as_str()),
                sort_order,
                false,
            )?);
        }
        cells.push(AutoloopMatrixCell::try_new(
            theme_id,
            role_id,
            mapping_id.clone(),
            AutoloopEntryId::try_new(format!(
                "theme-{}--{}",
                theme_id.value(),
                mapping_id.as_str()
            ))
            .map_err(|_| AutoloopCatalogError::IdentifierOverflow)?,
            display_name,
        )?);
        variants = prune_unused_variants(variants, &cells)?;
        sort_cells(&mut cells, &self.themes, &variants);
        self.revised(self.themes.clone(), variants, cells)
    }

    fn cell(
        &self,
        theme_id: ThemeId,
        role_id: &PhraseRoleId,
        variant_id: &VariantId,
    ) -> Option<&AutoloopMatrixCell> {
        self.cells.iter().find(|cell| {
            cell.theme_id() == theme_id
                && cell.role_id() == role_id
                && cell.variant_id() == variant_id
        })
    }

    fn require_active_role_variant(
        &self,
        role_id: &PhraseRoleId,
        variant_id: &VariantId,
    ) -> Result<&AutoloopVariant, AutoloopCatalogError> {
        self.variants
            .iter()
            .find(|variant| {
                variant.role_id() == role_id && variant.id() == variant_id && !variant.is_archived()
            })
            .ok_or(AutoloopCatalogError::IncompatibleVariant)
    }

    fn resolve_exact_strategy(
        &self,
        theme_id: ThemeId,
        role_id: &PhraseRoleId,
        variant_id: &VariantId,
        reason: AutoloopResolutionReason,
    ) -> Result<AutoloopResolution, AutoloopCatalogError> {
        self.require_active_role_variant(role_id, variant_id)?;
        let cell = self
            .cell(theme_id, role_id, variant_id)
            .ok_or(AutoloopCatalogError::MissingExactCell)?;
        Ok(self.resolution(cell, reason))
    }

    fn resolution(
        &self,
        cell: &AutoloopMatrixCell,
        reason: AutoloopResolutionReason,
    ) -> AutoloopResolution {
        AutoloopResolution {
            theme_id: cell.theme_id(),
            role_id: cell.role_id().clone(),
            variant_id: cell.variant_id().clone(),
            entry_id: cell.entry_id().clone(),
            display_name: cell.display_name().to_owned(),
            catalog_revision: self.revision,
            reason,
        }
    }

    fn revised(
        &self,
        themes: Vec<AutoloopTheme>,
        variants: Vec<AutoloopVariant>,
        cells: Vec<AutoloopMatrixCell>,
    ) -> Result<Self, AutoloopCatalogError> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(AutoloopCatalogError::ArithmeticOverflow)?;
        Self::try_new(revision, self.defaults_version, themes, variants, cells)
    }

    fn variant_index(
        &self,
        role_id: &PhraseRoleId,
        variant_id: &VariantId,
    ) -> Result<usize, AutoloopCatalogError> {
        self.variants
            .iter()
            .position(|variant| variant.role_id() == role_id && variant.id() == variant_id)
            .ok_or(AutoloopCatalogError::UnknownVariant)
    }

    fn ensure_unique_variant_name(
        &self,
        role_id: &PhraseRoleId,
        display_name: &str,
        except: Option<&VariantId>,
    ) -> Result<(), AutoloopCatalogError> {
        if self.variants.iter().any(|variant| {
            variant.role_id() == role_id
                && Some(variant.id()) != except
                && variant.display_name().eq_ignore_ascii_case(display_name)
        }) {
            return Err(AutoloopCatalogError::DuplicateDisplayName);
        }
        Ok(())
    }
}

fn validate_themes(themes: &[AutoloopTheme]) -> Result<(), AutoloopCatalogError> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for (index, theme) in themes.iter().enumerate() {
        if !ids.insert(theme.id()) {
            return Err(AutoloopCatalogError::DuplicateThemeId);
        }
        if !names.insert(theme.display_name().to_ascii_lowercase()) {
            return Err(AutoloopCatalogError::DuplicateDisplayName);
        }
        if usize::from(theme.sort_order()) != index + 1 {
            return Err(AutoloopCatalogError::InvalidSortOrder);
        }
    }
    Ok(())
}

fn validate_variants(variants: &[AutoloopVariant]) -> Result<(), AutoloopCatalogError> {
    let mut keys = HashSet::new();
    let mut role_names = HashSet::new();
    let mut orders = HashMap::<&str, u16>::new();
    for variant in variants {
        if !keys.insert((variant.role_id().as_str(), variant.id().as_str())) {
            return Err(AutoloopCatalogError::DuplicateVariantId);
        }
        if !role_names.insert((
            variant.role_id().as_str(),
            variant.display_name().to_ascii_lowercase(),
        )) {
            return Err(AutoloopCatalogError::DuplicateDisplayName);
        }
        let expected = orders
            .entry(variant.role_id().as_str())
            .and_modify(|value| *value += 1)
            .or_insert(1);
        if variant.sort_order() != *expected {
            return Err(AutoloopCatalogError::InvalidSortOrder);
        }
    }
    Ok(())
}

fn catalog_mapping_number(variant_id: &VariantId) -> Option<u16> {
    variant_id
        .as_str()
        .strip_prefix("mapping-")?
        .parse::<u16>()
        .ok()
}

fn prune_unused_variants(
    variants: Vec<AutoloopVariant>,
    cells: &[AutoloopMatrixCell],
) -> Result<Vec<AutoloopVariant>, AutoloopCatalogError> {
    let mut retained = variants
        .into_iter()
        .filter(|variant| {
            cells.iter().any(|cell| {
                cell.role_id() == variant.role_id() && cell.variant_id() == variant.id()
            })
        })
        .collect::<Vec<_>>();
    sort_variants(&mut retained);
    let mut next_orders = HashMap::<String, u16>::new();
    for variant in &mut retained {
        let next = next_orders
            .entry(variant.role_id().as_str().to_owned())
            .and_modify(|value| *value += 1)
            .or_insert(1);
        *variant = variant.rebuilt(
            variant.display_name().to_owned(),
            *next,
            variant.is_archived(),
        )?;
    }
    Ok(retained)
}

fn sort_variants(variants: &mut [AutoloopVariant]) {
    variants.sort_by(|left, right| {
        left.role_id()
            .cmp(right.role_id())
            .then_with(|| left.sort_order().cmp(&right.sort_order()))
            .then_with(|| left.id().cmp(right.id()))
    });
}

fn sort_cells(
    cells: &mut [AutoloopMatrixCell],
    themes: &[AutoloopTheme],
    variants: &[AutoloopVariant],
) {
    let theme_orders = themes
        .iter()
        .map(|theme| (theme.id(), theme.sort_order()))
        .collect::<HashMap<_, _>>();
    let variant_orders = variants
        .iter()
        .map(|variant| {
            (
                (variant.role_id().as_str(), variant.id().as_str()),
                variant.sort_order(),
            )
        })
        .collect::<HashMap<_, _>>();
    cells.sort_by(|left, right| {
        theme_orders
            .get(&left.theme_id())
            .cmp(&theme_orders.get(&right.theme_id()))
            .then_with(|| left.role_id().cmp(right.role_id()))
            .then_with(|| {
                variant_orders
                    .get(&(left.role_id().as_str(), left.variant_id().as_str()))
                    .cmp(
                        &variant_orders
                            .get(&(right.role_id().as_str(), right.variant_id().as_str())),
                    )
            })
    });
}

fn generated_entry_id(
    theme_id: ThemeId,
    role_id: &PhraseRoleId,
    variant_id: &VariantId,
) -> Result<AutoloopEntryId, AutoloopCatalogError> {
    AutoloopEntryId::try_new(format!(
        "theme-{}--{}--{}",
        theme_id.value(),
        role_id.as_str(),
        variant_id.as_str()
    ))
    .map_err(|_| AutoloopCatalogError::IdentifierOverflow)
}

fn validated_name(value: String) -> Result<String, AutoloopCatalogError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(AutoloopCatalogError::EmptyDisplayName);
    }
    if value.len() > 100 {
        return Err(AutoloopCatalogError::DisplayNameTooLong);
    }
    Ok(value)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AutoloopCatalogError {
    InvalidRevision,
    RevisionConflict { expected: u64, actual: u64 },
    InvalidThemeId,
    InvalidThemeCount,
    EmptyDisplayName,
    DisplayNameTooLong,
    InvalidSortOrder,
    DuplicateThemeId,
    DuplicateVariantId,
    DuplicateDisplayName,
    DuplicateCell,
    DuplicateEntryId,
    UnknownTheme,
    UnknownVariant,
    IncompatibleVariant,
    UnknownPhraseRole,
    MissingRoleCoverage,
    MissingExactCell,
    NoActiveVariantsForRole,
    NoChange,
    IdentifierOverflow,
    TooManyVariants,
    ArithmeticOverflow,
    DuplicateMappingIdentity,
}

impl std::fmt::Display for AutoloopCatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RevisionConflict { expected, actual } => write!(
                formatter,
                "Autoloop catalog revision {expected} is stale; current revision is {actual}"
            ),
            Self::MissingRoleCoverage => {
                formatter.write_str("the selected Theme has no entry for this Phrase Role")
            }
            value => write!(formatter, "invalid Autoloop catalog: {value:?}"),
        }
    }
}

impl std::error::Error for AutoloopCatalogError {}
