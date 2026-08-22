use std::collections::HashSet;

use lumi_domain::TrackId;

use crate::PhraseRoleId;

pub const PHRASE_ROLE_DEFAULTS_VERSION: u16 = 2;
pub const DEFAULT_CUSTOM_PHRASE_ROLE_COLOR_RGB: u32 = 0x33AD99;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhraseRole {
    id: PhraseRoleId,
    display_name: String,
    sort_order: u16,
    archived: bool,
    color_rgb: u32,
}

impl PhraseRole {
    pub fn try_new(
        id: PhraseRoleId,
        display_name: impl Into<String>,
        sort_order: u16,
        archived: bool,
    ) -> Result<Self, PhraseRoleCatalogError> {
        let color_rgb = default_phrase_role_color_rgb(id.as_str());
        Self::try_new_with_color_rgb(id, display_name, sort_order, archived, color_rgb)
    }

    pub fn try_new_with_color_rgb(
        id: PhraseRoleId,
        display_name: impl Into<String>,
        sort_order: u16,
        archived: bool,
        color_rgb: u32,
    ) -> Result<Self, PhraseRoleCatalogError> {
        let display_name = validated_display_name(display_name.into())?;
        if sort_order == 0 {
            return Err(PhraseRoleCatalogError::InvalidSortOrder);
        }
        if color_rgb > 0xFF_FF_FF {
            return Err(PhraseRoleCatalogError::InvalidColor);
        }
        Ok(Self {
            id,
            display_name,
            sort_order,
            archived,
            color_rgb,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &PhraseRoleId {
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

    #[must_use]
    pub const fn color_rgb(&self) -> u32 {
        self.color_rgb
    }

    fn with_display_name(&self, display_name: String) -> Result<Self, PhraseRoleCatalogError> {
        Self::try_new_with_color_rgb(
            self.id.clone(),
            display_name,
            self.sort_order,
            self.archived,
            self.color_rgb,
        )
    }

    fn with_sort_order(&self, sort_order: u16) -> Result<Self, PhraseRoleCatalogError> {
        Self::try_new_with_color_rgb(
            self.id.clone(),
            self.display_name.clone(),
            sort_order,
            self.archived,
            self.color_rgb,
        )
    }

    fn with_archived(&self, archived: bool) -> Result<Self, PhraseRoleCatalogError> {
        Self::try_new_with_color_rgb(
            self.id.clone(),
            self.display_name.clone(),
            self.sort_order,
            archived,
            self.color_rgb,
        )
    }

    fn with_color_rgb(&self, color_rgb: u32) -> Result<Self, PhraseRoleCatalogError> {
        Self::try_new_with_color_rgb(
            self.id.clone(),
            self.display_name.clone(),
            self.sort_order,
            self.archived,
            color_rgb,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePhraseMapping {
    provider_kind: String,
    raw_label: String,
    role_id: PhraseRoleId,
}

impl SourcePhraseMapping {
    pub fn try_new(
        provider_kind: impl Into<String>,
        raw_label: impl Into<String>,
        role_id: PhraseRoleId,
    ) -> Result<Self, PhraseRoleCatalogError> {
        let provider_kind = provider_kind.into().trim().to_owned();
        let raw_label = raw_label.into().trim().to_owned();
        if provider_kind.is_empty() {
            return Err(PhraseRoleCatalogError::EmptyProviderKind);
        }
        if provider_kind.len() > 64 {
            return Err(PhraseRoleCatalogError::ProviderKindTooLong);
        }
        if raw_label.is_empty() {
            return Err(PhraseRoleCatalogError::EmptyRawLabel);
        }
        if raw_label.len() > 100 {
            return Err(PhraseRoleCatalogError::RawLabelTooLong);
        }
        Ok(Self {
            provider_kind,
            raw_label,
            role_id,
        })
    }

    #[must_use]
    pub fn provider_kind(&self) -> &str {
        &self.provider_kind
    }

    #[must_use]
    pub fn raw_label(&self) -> &str {
        &self.raw_label
    }

    #[must_use]
    pub const fn role_id(&self) -> &PhraseRoleId {
        &self.role_id
    }

    #[must_use]
    pub fn normalized_label(&self) -> String {
        normalize_source_label(&self.raw_label)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhraseRoleCatalog {
    revision: u64,
    defaults_version: u16,
    roles: Vec<PhraseRole>,
    mappings: Vec<SourcePhraseMapping>,
}

impl PhraseRoleCatalog {
    pub fn try_new(
        revision: u64,
        defaults_version: u16,
        roles: Vec<PhraseRole>,
        mappings: Vec<SourcePhraseMapping>,
    ) -> Result<Self, PhraseRoleCatalogError> {
        if revision == 0 && (defaults_version != 0 || !mappings.is_empty()) {
            return Err(PhraseRoleCatalogError::InvalidRevision);
        }
        if revision > 0 && roles.is_empty() {
            return Err(PhraseRoleCatalogError::EmptyCatalog);
        }
        let mut role_ids = HashSet::new();
        let mut role_names = HashSet::new();
        for (index, role) in roles.iter().enumerate() {
            if !role_ids.insert(role.id().as_str().to_owned()) {
                return Err(PhraseRoleCatalogError::DuplicateRoleId);
            }
            if !role_names.insert(role.display_name().to_lowercase()) {
                return Err(PhraseRoleCatalogError::DuplicateDisplayName);
            }
            if usize::from(role.sort_order()) != index + 1 {
                return Err(PhraseRoleCatalogError::InvalidSortOrder);
            }
        }
        if !roles.is_empty() && roles.iter().all(PhraseRole::is_archived) {
            return Err(PhraseRoleCatalogError::NoActiveRoles);
        }
        let mut mapping_keys = HashSet::new();
        for mapping in &mappings {
            if !role_ids.contains(mapping.role_id().as_str()) {
                return Err(PhraseRoleCatalogError::UnknownRole);
            }
            if !mapping_keys.insert((
                mapping.provider_kind().to_ascii_lowercase(),
                mapping.normalized_label(),
            )) {
                return Err(PhraseRoleCatalogError::DuplicateMapping);
            }
        }
        Ok(Self {
            revision,
            defaults_version,
            roles,
            mappings,
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
    pub fn roles(&self) -> &[PhraseRole] {
        &self.roles
    }

    #[must_use]
    pub fn mappings(&self) -> &[SourcePhraseMapping] {
        &self.mappings
    }

    pub fn add_role(
        &self,
        display_name: impl Into<String>,
    ) -> Result<Self, PhraseRoleCatalogError> {
        let display_name = validated_display_name(display_name.into())?;
        self.ensure_unique_name(&display_name, None)?;
        let mut suffix = 1_u64;
        let id = loop {
            let candidate = PhraseRoleId::try_new(format!("custom-{suffix}"))
                .map_err(|_| PhraseRoleCatalogError::IdentifierOverflow)?;
            if !self.roles.iter().any(|role| role.id() == &candidate) {
                break candidate;
            }
            suffix = suffix
                .checked_add(1)
                .ok_or(PhraseRoleCatalogError::ArithmeticOverflow)?;
        };
        let order = u16::try_from(self.roles.len() + 1)
            .map_err(|_| PhraseRoleCatalogError::TooManyRoles)?;
        let mut roles = self.roles.clone();
        roles.push(PhraseRole::try_new(id, display_name, order, false)?);
        self.revised(roles, self.mappings.clone())
    }

    pub fn rename_role(
        &self,
        role_id: &PhraseRoleId,
        display_name: impl Into<String>,
    ) -> Result<Self, PhraseRoleCatalogError> {
        let display_name = validated_display_name(display_name.into())?;
        self.ensure_unique_name(&display_name, Some(role_id))?;
        let index = self.role_index(role_id)?;
        if self.roles[index].display_name() == display_name {
            return Err(PhraseRoleCatalogError::NoChange);
        }
        let mut roles = self.roles.clone();
        roles[index] = roles[index].with_display_name(display_name)?;
        self.revised(roles, self.mappings.clone())
    }

    pub fn move_role(
        &self,
        role_id: &PhraseRoleId,
        direction: PhraseRoleMove,
    ) -> Result<Self, PhraseRoleCatalogError> {
        let index = self.role_index(role_id)?;
        let other = match direction {
            PhraseRoleMove::Earlier => index.checked_sub(1),
            PhraseRoleMove::Later => index
                .checked_add(1)
                .filter(|value| *value < self.roles.len()),
        }
        .ok_or(PhraseRoleCatalogError::NoChange)?;
        let mut roles = self.roles.clone();
        roles.swap(index, other);
        for (position, role) in roles.clone().iter().enumerate() {
            roles[position] = role.with_sort_order(
                u16::try_from(position + 1).map_err(|_| PhraseRoleCatalogError::TooManyRoles)?,
            )?;
        }
        self.revised(roles, self.mappings.clone())
    }

    pub fn set_archived(
        &self,
        role_id: &PhraseRoleId,
        archived: bool,
    ) -> Result<Self, PhraseRoleCatalogError> {
        let index = self.role_index(role_id)?;
        if self.roles[index].is_archived() == archived {
            return Err(PhraseRoleCatalogError::NoChange);
        }
        let mut roles = self.roles.clone();
        roles[index] = roles[index].with_archived(archived)?;
        self.revised(roles, self.mappings.clone())
    }

    pub fn set_color_rgb(
        &self,
        role_id: &PhraseRoleId,
        color_rgb: u32,
    ) -> Result<Self, PhraseRoleCatalogError> {
        let index = self.role_index(role_id)?;
        if self.roles[index].color_rgb() == color_rgb {
            return Err(PhraseRoleCatalogError::NoChange);
        }
        let mut roles = self.roles.clone();
        roles[index] = roles[index].with_color_rgb(color_rgb)?;
        self.revised(roles, self.mappings.clone())
    }

    pub fn upsert_mapping(
        &self,
        mapping: SourcePhraseMapping,
    ) -> Result<Self, PhraseRoleCatalogError> {
        if !self.roles.iter().any(|role| role.id() == mapping.role_id()) {
            return Err(PhraseRoleCatalogError::UnknownRole);
        }
        let provider = mapping.provider_kind().to_ascii_lowercase();
        let label = mapping.normalized_label();
        let mut mappings = self.mappings.clone();
        if let Some(index) = mappings.iter().position(|candidate| {
            candidate.provider_kind().eq_ignore_ascii_case(&provider)
                && candidate.normalized_label() == label
        }) {
            if mappings[index] == mapping {
                return Err(PhraseRoleCatalogError::NoChange);
            }
            mappings[index] = mapping;
        } else {
            mappings.push(mapping);
            mappings.sort_by(|left, right| {
                left.provider_kind()
                    .cmp(right.provider_kind())
                    .then_with(|| left.normalized_label().cmp(&right.normalized_label()))
            });
        }
        self.revised(self.roles.clone(), mappings)
    }

    #[must_use]
    pub fn resolve(&self, provider_kind: &str, raw_label: &str) -> Option<&PhraseRoleId> {
        let normalized = normalize_source_label(raw_label);
        self.mappings
            .iter()
            .find(|mapping| {
                mapping.provider_kind().eq_ignore_ascii_case(provider_kind)
                    && mapping.normalized_label() == normalized
            })
            .or_else(|| {
                self.mappings.iter().find(|mapping| {
                    mapping.provider_kind().eq_ignore_ascii_case(provider_kind)
                        && mapping.normalized_label() == "*"
                })
            })
            .map(SourcePhraseMapping::role_id)
    }

    fn revised(
        &self,
        roles: Vec<PhraseRole>,
        mappings: Vec<SourcePhraseMapping>,
    ) -> Result<Self, PhraseRoleCatalogError> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(PhraseRoleCatalogError::ArithmeticOverflow)?;
        Self::try_new(revision, self.defaults_version, roles, mappings)
    }

    fn role_index(&self, role_id: &PhraseRoleId) -> Result<usize, PhraseRoleCatalogError> {
        self.roles
            .iter()
            .position(|role| role.id() == role_id)
            .ok_or(PhraseRoleCatalogError::UnknownRole)
    }

    fn ensure_unique_name(
        &self,
        display_name: &str,
        except: Option<&PhraseRoleId>,
    ) -> Result<(), PhraseRoleCatalogError> {
        if self.roles.iter().any(|role| {
            Some(role.id()) != except && role.display_name().eq_ignore_ascii_case(display_name)
        }) {
            return Err(PhraseRoleCatalogError::DuplicateDisplayName);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhraseRoleMove {
    Earlier,
    Later,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhraseRoleTrackUsage {
    track_id: TrackId,
    title: String,
    phrase_count: u64,
}

impl PhraseRoleTrackUsage {
    #[must_use]
    pub fn new(track_id: TrackId, title: impl Into<String>, phrase_count: u64) -> Self {
        Self {
            track_id,
            title: title.into(),
            phrase_count,
        }
    }

    #[must_use]
    pub const fn track_id(&self) -> TrackId {
        self.track_id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub const fn phrase_count(&self) -> u64 {
        self.phrase_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhraseRoleUsage {
    role_id: PhraseRoleId,
    phrase_count: u64,
    tracks: Vec<PhraseRoleTrackUsage>,
    catalog_row_count: u64,
}

impl PhraseRoleUsage {
    #[must_use]
    pub const fn new(
        role_id: PhraseRoleId,
        phrase_count: u64,
        tracks: Vec<PhraseRoleTrackUsage>,
        catalog_row_count: u64,
    ) -> Self {
        Self {
            role_id,
            phrase_count,
            tracks,
            catalog_row_count,
        }
    }

    #[must_use]
    pub const fn role_id(&self) -> &PhraseRoleId {
        &self.role_id
    }

    #[must_use]
    pub const fn phrase_count(&self) -> u64 {
        self.phrase_count
    }

    #[must_use]
    pub fn tracks(&self) -> &[PhraseRoleTrackUsage] {
        &self.tracks
    }

    #[must_use]
    pub const fn catalog_row_count(&self) -> u64 {
        self.catalog_row_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhraseRoleCatalogError {
    InvalidRevision,
    EmptyCatalog,
    EmptyDisplayName,
    DisplayNameTooLong,
    DuplicateDisplayName,
    DuplicateRoleId,
    InvalidSortOrder,
    InvalidColor,
    NoActiveRoles,
    UnknownRole,
    EmptyProviderKind,
    ProviderKindTooLong,
    EmptyRawLabel,
    RawLabelTooLong,
    DuplicateMapping,
    NoChange,
    TooManyRoles,
    IdentifierOverflow,
    ArithmeticOverflow,
}

impl std::fmt::Display for PhraseRoleCatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRevision => "phrase-role catalog revision is invalid",
            Self::EmptyCatalog => "phrase-role catalog may not be empty",
            Self::EmptyDisplayName => "phrase-role display name may not be empty",
            Self::DisplayNameTooLong => "phrase-role display name exceeds 80 bytes",
            Self::DuplicateDisplayName => "phrase-role display names must be unique",
            Self::DuplicateRoleId => "phrase-role IDs must be unique",
            Self::InvalidSortOrder => "phrase-role ordering must be contiguous and start at one",
            Self::InvalidColor => "phrase-role color must be a 24-bit sRGB value",
            Self::NoActiveRoles => "at least one phrase role must remain active",
            Self::UnknownRole => "phrase role does not exist",
            Self::EmptyProviderKind => "source provider kind may not be empty",
            Self::ProviderKindTooLong => "source provider kind exceeds 64 bytes",
            Self::EmptyRawLabel => "source phrase label may not be empty",
            Self::RawLabelTooLong => "source phrase label exceeds 100 bytes",
            Self::DuplicateMapping => "source phrase mapping already exists",
            Self::NoChange => "phrase-role change has no effect",
            Self::TooManyRoles => "phrase-role catalog contains too many roles",
            Self::IdentifierOverflow => "phrase-role identifier could not be generated",
            Self::ArithmeticOverflow => "phrase-role catalog arithmetic overflow",
        })
    }
}

#[must_use]
pub fn default_phrase_role_color_rgb(role_id: &str) -> u32 {
    match role_id {
        "intro-outro" | "intro" | "outro" => 0x408CF2,
        "bridge" => 0x5E6BC7,
        "breakdown-1" | "breakdown-2" | "breakdown-3" | "breakdown" => 0x7A47D4,
        "synth" => 0xD13DB8,
        "pre-drop" => 0xF27433,
        "buildup-1" | "buildup-2" | "buildup-3" | "build" => 0xF5A81F,
        "drop" => 0xEB3342,
        _ => DEFAULT_CUSTOM_PHRASE_ROLE_COLOR_RGB,
    }
}

impl std::error::Error for PhraseRoleCatalogError {}

#[must_use]
pub fn normalize_source_label(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn validated_display_name(value: String) -> Result<String, PhraseRoleCatalogError> {
    let trimmed = value.trim().to_owned();
    if trimmed.is_empty() {
        return Err(PhraseRoleCatalogError::EmptyDisplayName);
    }
    if trimmed.len() > 80 {
        return Err(PhraseRoleCatalogError::DisplayNameTooLong);
    }
    Ok(trimmed)
}
