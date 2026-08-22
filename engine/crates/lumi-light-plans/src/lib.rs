//! Deterministic, I/O-free compilation of Lumi Light Plans.
//!
//! This crate deliberately knows nothing about Pro DJ Link, Ableton Link,
//! CoreMIDI, databases or UI state. Its output is compiled before playback and
//! can be consumed by the existing realtime executor as immutable addresses.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_THEME_COOLDOWN_TRACKS: u8 = 1;
pub const DEFAULT_AUTOLOOP_COOLDOWN_USES: u8 = 2;
pub const DEFAULT_DUPLICATE_PLAN_WINDOW: u8 = 4;
pub const DEFAULT_SELECTION_WEIGHT: u8 = 2;
const HISTORY_LIMIT: usize = 32;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorBehavior {
    Neutral,
    Prefer,
    Only,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModifierKind {
    Atmosphere,
    Color,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModifierScope {
    Phrase,
    Track,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoloopRule {
    pub theme_id: u64,
    pub role_id: String,
    pub variant_id: String,
    pub enabled: bool,
    pub selection_weight: u8,
    pub color_behavior: ColorBehavior,
    pub color_rgb: Vec<u32>,
}

impl AutoloopRule {
    #[must_use]
    pub fn effective_weight(&self) -> u8 {
        self.selection_weight.clamp(1, 4)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputModifier {
    pub id: String,
    pub provider_kind: String,
    pub kind: ModifierKind,
    pub display_name: String,
    pub enabled: bool,
    pub midi_channel: u8,
    pub midi_note: u8,
    pub activation_verified: bool,
    pub release_verified: bool,
}

impl OutputModifier {
    #[must_use]
    pub const fn automatic_execution_ready(&self) -> bool {
        self.enabled && self.activation_verified && self.release_verified
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifierRule {
    pub modifier_id: String,
    pub role_id: String,
    pub application_rate: u8,
    pub selection_weight: u8,
    pub cooldown_uses: u8,
    pub scope: ModifierScope,
    pub color_behavior: ColorBehavior,
    pub color_rgb: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightPlanningPolicy {
    pub revision: u64,
    pub theme_cooldown_tracks: u8,
    pub autoloop_cooldown_uses: u8,
    pub duplicate_plan_window: u8,
    pub rules: Vec<AutoloopRule>,
    pub modifiers: Vec<OutputModifier>,
    pub modifier_rules: Vec<ModifierRule>,
}

impl Default for LightPlanningPolicy {
    fn default() -> Self {
        Self {
            revision: 1,
            theme_cooldown_tracks: DEFAULT_THEME_COOLDOWN_TRACKS,
            autoloop_cooldown_uses: DEFAULT_AUTOLOOP_COOLDOWN_USES,
            duplicate_plan_window: DEFAULT_DUPLICATE_PLAN_WINDOW,
            rules: Vec::new(),
            modifiers: Vec::new(),
            modifier_rules: Vec::new(),
        }
    }
}

impl LightPlanningPolicy {
    pub fn validate(&self) -> Result<(), LightPlanError> {
        if self.revision == 0 {
            return Err(LightPlanError::InvalidRevision);
        }
        let mut keys = BTreeSet::new();
        for rule in &self.rules {
            if rule.theme_id == 0
                || rule.role_id.trim().is_empty()
                || rule.variant_id.trim().is_empty()
            {
                return Err(LightPlanError::InvalidRule);
            }
            if !(1..=4).contains(&rule.selection_weight) {
                return Err(LightPlanError::InvalidWeight);
            }
            if !keys.insert((
                rule.theme_id,
                rule.role_id.as_str(),
                rule.variant_id.as_str(),
            )) {
                return Err(LightPlanError::DuplicateRule);
            }
        }
        let mut modifier_ids = BTreeSet::new();
        for modifier in &self.modifiers {
            if modifier.id.trim().is_empty()
                || modifier.display_name.trim().is_empty()
                || !(1..=16).contains(&modifier.midi_channel)
            {
                return Err(LightPlanError::InvalidModifier);
            }
            if !modifier_ids.insert(modifier.id.as_str()) {
                return Err(LightPlanError::DuplicateModifier);
            }
        }
        for rule in &self.modifier_rules {
            if !modifier_ids.contains(rule.modifier_id.as_str())
                || rule.role_id.trim().is_empty()
                || rule.application_rate > 100
                || !(1..=4).contains(&rule.selection_weight)
            {
                return Err(LightPlanError::InvalidModifierRule);
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn rule_for(
        &self,
        theme_id: u64,
        role_id: &str,
        variant_id: &str,
    ) -> Option<&AutoloopRule> {
        self.rules.iter().find(|rule| {
            rule.theme_id == theme_id && rule.role_id == role_id && rule.variant_id == variant_id
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub theme_id: u64,
    pub role_id: String,
    pub variant_id: String,
    pub entry_id: String,
    pub display_name: String,
    pub autoloop_number: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhraseSelection {
    Automatic,
    FixedVariant(String),
    PlanOverride(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhraseRequest {
    pub phrase_index: u16,
    pub role_id: String,
    pub selection: PhraseSelection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionEvidence {
    pub reason: String,
    pub effective_weight: u8,
    pub color_influence: String,
    pub repeat_protection: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledAutoloopChoice {
    pub phrase_index: u16,
    pub role_id: String,
    pub variant_id: String,
    pub entry_id: String,
    pub display_name: String,
    pub autoloop_number: u16,
    pub evidence: SelectionEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledModifierChoice {
    pub phrase_index: u16,
    pub role_id: String,
    pub modifier_id: String,
    pub kind: ModifierKind,
    pub scope: ModifierScope,
    pub display_name: String,
    pub provider_kind: String,
    pub midi_channel: u8,
    pub midi_note: u8,
    pub evidence: SelectionEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledLightPlan {
    pub policy_revision: u64,
    pub variation_seed: u64,
    pub theme_id: u64,
    pub choices: Vec<CompiledAutoloopChoice>,
    pub modifier_choices: Vec<CompiledModifierChoice>,
    pub signature: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VariationHistory {
    committed: VecDeque<PlanHistoryEntry>,
    reserved: BTreeMap<String, PlanHistoryEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlanHistoryEntry {
    theme_id: u64,
    signature: u64,
    by_role: BTreeMap<String, Vec<String>>,
    modifiers: Vec<String>,
}

impl VariationHistory {
    pub fn reserve(&mut self, reservation_id: impl Into<String>, plan: &CompiledLightPlan) {
        self.reserved
            .insert(reservation_id.into(), history_entry(plan));
    }

    pub fn release(&mut self, reservation_id: &str) {
        self.reserved.remove(reservation_id);
    }

    pub fn commit(&mut self, reservation_id: &str) {
        if let Some(entry) = self.reserved.remove(reservation_id) {
            self.committed.push_back(entry);
            while self.committed.len() > HISTORY_LIMIT {
                self.committed.pop_front();
            }
        }
    }

    pub fn clear(&mut self) {
        self.committed.clear();
        self.reserved.clear();
    }

    fn recent_variants(&self, role_id: &str, limit: usize) -> BTreeSet<&str> {
        self.committed
            .iter()
            .rev()
            .chain(self.reserved.values().rev())
            .filter_map(|entry| entry.by_role.get(role_id))
            .flat_map(|values| values.iter().rev())
            .take(limit)
            .map(String::as_str)
            .collect()
    }

    fn recent_signatures(&self, limit: usize) -> BTreeSet<u64> {
        self.committed
            .iter()
            .rev()
            .chain(self.reserved.values().rev())
            .take(limit)
            .map(|entry| entry.signature)
            .collect()
    }

    fn recent_modifiers(&self, limit: usize) -> BTreeSet<&str> {
        self.committed
            .iter()
            .rev()
            .chain(self.reserved.values().rev())
            .flat_map(|entry| entry.modifiers.iter().rev())
            .take(limit)
            .map(String::as_str)
            .collect()
    }

    #[must_use]
    pub fn theme_is_recent(&self, theme_id: u64, limit: usize) -> bool {
        self.committed
            .iter()
            .rev()
            .chain(self.reserved.values().rev())
            .take(limit)
            .any(|entry| entry.theme_id == theme_id)
    }
}

pub fn compile(
    policy: &LightPlanningPolicy,
    theme_id: u64,
    track_color_rgb: Option<u32>,
    variation_seed: u64,
    phrases: &[PhraseRequest],
    candidates: &[Candidate],
    history: &VariationHistory,
) -> Result<CompiledLightPlan, LightPlanError> {
    policy.validate()?;
    let mut choices = Vec::with_capacity(phrases.len());
    let mut local_role_choices: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for phrase in phrases {
        let mut eligible = candidates
            .iter()
            .filter(|candidate| {
                candidate.theme_id == theme_id && candidate.role_id == phrase.role_id
            })
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            return Err(LightPlanError::MissingCandidate {
                role_id: phrase.role_id.clone(),
            });
        }
        let requested = match &phrase.selection {
            PhraseSelection::Automatic => None,
            PhraseSelection::FixedVariant(id) | PhraseSelection::PlanOverride(id) => Some(id),
        };
        let (selected, evidence) = if let Some(variant_id) = requested {
            let selected = eligible
                .into_iter()
                .find(|candidate| &candidate.variant_id == variant_id)
                .ok_or_else(|| LightPlanError::MissingVariant {
                    role_id: phrase.role_id.clone(),
                    variant_id: variant_id.clone(),
                })?;
            let reason = match phrase.selection {
                PhraseSelection::PlanOverride(_) => "plan override",
                _ => "track fixed variant",
            };
            (
                selected,
                SelectionEvidence {
                    reason: reason.to_owned(),
                    effective_weight: 4,
                    color_influence: "not evaluated for explicit choice".to_owned(),
                    repeat_protection: "explicit choice takes precedence".to_owned(),
                },
            )
        } else {
            eligible.retain(|candidate| {
                policy
                    .rule_for(theme_id, &phrase.role_id, &candidate.variant_id)
                    .is_none_or(|rule| rule.enabled)
            });
            if eligible.is_empty() {
                return Err(LightPlanError::MissingCandidate {
                    role_id: phrase.role_id.clone(),
                });
            }
            let matching_only = eligible
                .iter()
                .copied()
                .filter(|candidate| {
                    policy
                        .rule_for(theme_id, &phrase.role_id, &candidate.variant_id)
                        .is_some_and(|rule| {
                            rule.color_behavior == ColorBehavior::Only
                                && track_color_rgb
                                    .is_some_and(|color| rule.color_rgb.contains(&color))
                        })
                })
                .collect::<Vec<_>>();
            if !matching_only.is_empty() {
                eligible = matching_only;
            } else {
                eligible.retain(|candidate| {
                    policy
                        .rule_for(theme_id, &phrase.role_id, &candidate.variant_id)
                        .is_none_or(|rule| rule.color_behavior != ColorBehavior::Only)
                });
            }
            if eligible.is_empty() {
                return Err(LightPlanError::MissingColorCandidate {
                    role_id: phrase.role_id.clone(),
                });
            }
            let history_recent = history
                .recent_variants(&phrase.role_id, usize::from(policy.autoloop_cooldown_uses));
            let local_recent = local_role_choices
                .get(phrase.role_id.as_str())
                .into_iter()
                .flat_map(|values| values.iter().rev())
                .take(usize::from(policy.autoloop_cooldown_uses))
                .copied()
                .collect::<BTreeSet<_>>();
            let unrepeated = eligible
                .iter()
                .copied()
                .filter(|candidate| {
                    !history_recent.contains(candidate.variant_id.as_str())
                        && !local_recent.contains(candidate.variant_id.as_str())
                })
                .collect::<Vec<_>>();
            let repeat_relaxed = unrepeated.is_empty();
            if !repeat_relaxed {
                eligible = unrepeated;
            }
            let weighted = eligible
                .iter()
                .map(|candidate| {
                    let rule = policy.rule_for(theme_id, &phrase.role_id, &candidate.variant_id);
                    let base =
                        rule.map_or(DEFAULT_SELECTION_WEIGHT, AutoloopRule::effective_weight);
                    let preferred = rule.is_some_and(|rule| {
                        rule.color_behavior == ColorBehavior::Prefer
                            && track_color_rgb.is_some_and(|color| rule.color_rgb.contains(&color))
                    });
                    (
                        *candidate,
                        u64::from(if preferred {
                            base.saturating_mul(2)
                        } else {
                            base
                        }),
                        preferred,
                    )
                })
                .collect::<Vec<_>>();
            let total = weighted.iter().map(|(_, weight, _)| *weight).sum::<u64>();
            let roll = stable_mix(
                variation_seed
                    ^ u64::from(phrase.phrase_index)
                    ^ stable_text_hash(&phrase.role_id)
                    ^ policy.revision,
            ) % total.max(1);
            let mut cursor = 0_u64;
            let mut selected = weighted[0];
            for candidate in &weighted {
                cursor = cursor.saturating_add(candidate.1);
                if roll < cursor {
                    selected = *candidate;
                    break;
                }
            }
            (
                selected.0,
                SelectionEvidence {
                    reason: if matching_color_only(
                        policy,
                        theme_id,
                        &phrase.role_id,
                        &selected.0.variant_id,
                        track_color_rgb,
                    ) {
                        "track color only"
                    } else if selected.2 {
                        "track color preferred weighted variation"
                    } else {
                        "weighted variation"
                    }
                    .to_owned(),
                    effective_weight: u8::try_from(selected.1).unwrap_or(u8::MAX),
                    color_influence: if selected.2 {
                        "prefer match"
                    } else if matching_color_only(
                        policy,
                        theme_id,
                        &phrase.role_id,
                        &selected.0.variant_id,
                        track_color_rgb,
                    ) {
                        "only match"
                    } else {
                        "neutral"
                    }
                    .to_owned(),
                    repeat_protection: if repeat_relaxed {
                        "oldest constraint relaxed; Phrase Role preserved"
                    } else {
                        "recent variants excluded"
                    }
                    .to_owned(),
                },
            )
        };
        local_role_choices
            .entry(phrase.role_id.as_str())
            .or_default()
            .push(selected.variant_id.as_str());
        choices.push(CompiledAutoloopChoice {
            phrase_index: phrase.phrase_index,
            role_id: phrase.role_id.clone(),
            variant_id: selected.variant_id.clone(),
            entry_id: selected.entry_id.clone(),
            display_name: selected.display_name.clone(),
            autoloop_number: selected.autoloop_number,
            evidence,
        });
    }
    let modifier_choices =
        compile_modifiers(policy, track_color_rgb, variation_seed, phrases, history);

    // A plan signature represents the observable result, not the random seed
    // that produced it. This makes whole-plan repeat protection catch two
    // different seeds that happen to select the same AutoLoops.
    let mut signature = stable_mix(theme_id);
    for choice in &choices {
        signature = stable_mix(signature ^ stable_text_hash(&choice.variant_id));
    }
    for phrase in phrases {
        for kind in [ModifierKind::Atmosphere, ModifierKind::Color] {
            let selected = modifier_choices
                .iter()
                .find(|choice| choice.phrase_index == phrase.phrase_index && choice.kind == kind);
            signature = stable_mix(
                signature
                    ^ selected.map_or(0, |choice| stable_text_hash(&choice.modifier_id))
                    ^ match kind {
                        ModifierKind::Atmosphere => 0xa710_5001,
                        ModifierKind::Color => 0xc010_7002,
                    },
            );
        }
    }
    let recent_signatures = history.recent_signatures(usize::from(policy.duplicate_plan_window));
    if recent_signatures.contains(&signature) {
        let selection_history = history.without_signatures();
        for retry in 1..=32_u64 {
            let alternative = compile(
                policy,
                theme_id,
                track_color_rgb,
                variation_seed.wrapping_add(retry),
                phrases,
                candidates,
                &selection_history,
            )?;
            if !recent_signatures.contains(&alternative.signature) {
                return Ok(alternative);
            }
        }
        // A sparse catalog can have exactly one valid whole plan. Preserve the
        // Phrase Role invariant and relax only the impossible oldest rule.
    }
    Ok(CompiledLightPlan {
        policy_revision: policy.revision,
        variation_seed,
        theme_id,
        choices,
        modifier_choices,
        signature,
    })
}

fn compile_modifiers(
    policy: &LightPlanningPolicy,
    track_color_rgb: Option<u32>,
    variation_seed: u64,
    phrases: &[PhraseRequest],
    history: &VariationHistory,
) -> Vec<CompiledModifierChoice> {
    let mut choices = Vec::new();
    for kind in [ModifierKind::Atmosphere, ModifierKind::Color] {
        let mut carried: Option<CompiledModifierChoice> = None;
        let mut local_uses: Vec<String> = Vec::new();
        for phrase in phrases {
            if let Some(active) = &carried {
                let mut continued = active.clone();
                continued.phrase_index = phrase.phrase_index;
                continued.role_id = phrase.role_id.clone();
                continued.evidence.reason = "whole-track modifier continued".to_owned();
                choices.push(continued);
                continue;
            }
            let mut eligible = policy
                .modifier_rules
                .iter()
                .filter_map(|rule| {
                    let modifier = policy.modifiers.iter().find(|modifier| {
                        modifier.id == rule.modifier_id
                            && modifier.kind == kind
                            && modifier.automatic_execution_ready()
                    })?;
                    (rule.role_id == phrase.role_id).then_some((modifier, rule))
                })
                .filter(|(modifier, rule)| {
                    let history_recent = history.recent_modifiers(usize::from(rule.cooldown_uses));
                    let local_recent = local_uses
                        .iter()
                        .rev()
                        .take(usize::from(rule.cooldown_uses))
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>();
                    !history_recent.contains(modifier.id.as_str())
                        && !local_recent.contains(modifier.id.as_str())
                })
                .collect::<Vec<_>>();
            let matching_only = eligible
                .iter()
                .copied()
                .filter(|(_, rule)| {
                    rule.color_behavior == ColorBehavior::Only
                        && track_color_rgb.is_some_and(|color| rule.color_rgb.contains(&color))
                })
                .collect::<Vec<_>>();
            if matching_only.is_empty() {
                eligible.retain(|(_, rule)| rule.color_behavior != ColorBehavior::Only);
            } else {
                eligible = matching_only;
            }
            eligible.retain(|(modifier, rule)| {
                if rule.application_rate == 0 {
                    return false;
                }
                let roll = stable_mix(
                    variation_seed
                        ^ u64::from(phrase.phrase_index)
                        ^ stable_text_hash(&modifier.id)
                        ^ policy.revision
                        ^ 0x4d4f_4449_4649_4552,
                ) % 100;
                roll < u64::from(rule.application_rate)
            });
            if eligible.is_empty() {
                continue;
            }
            let weighted = eligible
                .iter()
                .map(|(modifier, rule)| {
                    let preferred = rule.color_behavior == ColorBehavior::Prefer
                        && track_color_rgb.is_some_and(|color| rule.color_rgb.contains(&color));
                    let weight = rule.selection_weight.clamp(1, 4);
                    (
                        *modifier,
                        *rule,
                        if preferred {
                            weight.saturating_mul(2)
                        } else {
                            weight
                        },
                        preferred,
                    )
                })
                .collect::<Vec<_>>();
            let total = weighted
                .iter()
                .map(|(_, _, weight, _)| u64::from(*weight))
                .sum::<u64>();
            let roll = stable_mix(
                variation_seed
                    ^ u64::from(phrase.phrase_index)
                    ^ policy.revision
                    ^ match kind {
                        ModifierKind::Atmosphere => 0xa710_5001,
                        ModifierKind::Color => 0xc010_7002,
                    },
            ) % total.max(1);
            let mut cursor = 0_u64;
            let mut selected = weighted[0];
            for candidate in &weighted {
                cursor = cursor.saturating_add(u64::from(candidate.2));
                if roll < cursor {
                    selected = *candidate;
                    break;
                }
            }
            let (modifier, rule, effective_weight, preferred) = selected;
            let choice = CompiledModifierChoice {
                phrase_index: phrase.phrase_index,
                role_id: phrase.role_id.clone(),
                modifier_id: modifier.id.clone(),
                kind,
                scope: rule.scope,
                display_name: modifier.display_name.clone(),
                provider_kind: modifier.provider_kind.clone(),
                midi_channel: modifier.midi_channel,
                midi_note: modifier.midi_note,
                evidence: SelectionEvidence {
                    reason: match rule.scope {
                        ModifierScope::Phrase => "application rate selected phrase modifier",
                        ModifierScope::Track => "application rate selected whole-track modifier",
                    }
                    .to_owned(),
                    effective_weight,
                    color_influence: if preferred {
                        "prefer match"
                    } else if rule.color_behavior == ColorBehavior::Only {
                        "only match"
                    } else {
                        "neutral"
                    }
                    .to_owned(),
                    repeat_protection: "modifier cooldown satisfied".to_owned(),
                },
            };
            local_uses.push(modifier.id.clone());
            if rule.scope == ModifierScope::Track {
                carried = Some(choice.clone());
            }
            choices.push(choice);
        }
    }
    choices.sort_by_key(|choice| (choice.phrase_index, choice.kind));
    choices
}

impl VariationHistory {
    fn without_signatures(&self) -> Self {
        let mut clone = self.clone();
        for entry in clone
            .committed
            .iter_mut()
            .chain(clone.reserved.values_mut())
        {
            entry.signature = 0;
        }
        clone
    }
}

fn matching_color_only(
    policy: &LightPlanningPolicy,
    theme_id: u64,
    role_id: &str,
    variant_id: &str,
    track_color_rgb: Option<u32>,
) -> bool {
    policy
        .rule_for(theme_id, role_id, variant_id)
        .is_some_and(|rule| {
            rule.color_behavior == ColorBehavior::Only
                && track_color_rgb.is_some_and(|color| rule.color_rgb.contains(&color))
        })
}

fn history_entry(plan: &CompiledLightPlan) -> PlanHistoryEntry {
    let mut by_role = BTreeMap::new();
    for choice in &plan.choices {
        by_role
            .entry(choice.role_id.clone())
            .or_insert_with(Vec::new)
            .push(choice.variant_id.clone());
    }
    let mut track_modifiers = BTreeSet::new();
    let modifiers = plan
        .modifier_choices
        .iter()
        .filter(|choice| {
            choice.scope == ModifierScope::Phrase
                || track_modifiers.insert(choice.modifier_id.as_str())
        })
        .map(|choice| choice.modifier_id.clone())
        .collect();
    PlanHistoryEntry {
        theme_id: plan.theme_id,
        signature: plan.signature,
        by_role,
        modifiers,
    }
}

fn stable_text_hash(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

fn stable_mix(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LightPlanError {
    #[error("light planning policy revision must be positive")]
    InvalidRevision,
    #[error("invalid AutoLoop planning rule")]
    InvalidRule,
    #[error("selection weight must be between 1 and 4")]
    InvalidWeight,
    #[error("duplicate AutoLoop planning rule")]
    DuplicateRule,
    #[error("invalid output modifier")]
    InvalidModifier,
    #[error("duplicate output modifier")]
    DuplicateModifier,
    #[error("invalid modifier planning rule")]
    InvalidModifierRule,
    #[error("no mapped AutoLoop candidate for Phrase Role {role_id}")]
    MissingCandidate { role_id: String },
    #[error("no color-eligible AutoLoop candidate for Phrase Role {role_id}")]
    MissingColorCandidate { role_id: String },
    #[error("variant {variant_id} is not mapped for Phrase Role {role_id}")]
    MissingVariant { role_id: String, variant_id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(variant: &str, number: u16) -> Candidate {
        Candidate {
            theme_id: 1,
            role_id: "drop".to_owned(),
            variant_id: variant.to_owned(),
            entry_id: format!("entry-{variant}"),
            display_name: variant.to_owned(),
            autoloop_number: number,
        }
    }

    fn phrase(index: u16) -> PhraseRequest {
        PhraseRequest {
            phrase_index: index,
            role_id: "drop".to_owned(),
            selection: PhraseSelection::Automatic,
        }
    }

    #[test]
    fn same_seed_is_deterministic() -> Result<(), LightPlanError> {
        let policy = LightPlanningPolicy::default();
        let candidates = [candidate("a", 1), candidate("b", 2), candidate("c", 3)];
        let first = compile(
            &policy,
            1,
            None,
            42,
            &[phrase(0), phrase(1)],
            &candidates,
            &VariationHistory::default(),
        )?;
        let second = compile(
            &policy,
            1,
            None,
            42,
            &[phrase(0), phrase(1)],
            &candidates,
            &VariationHistory::default(),
        )?;
        assert_eq!(first, second);
        Ok(())
    }

    #[test]
    fn color_only_restricts_candidates() -> Result<(), LightPlanError> {
        let mut policy = LightPlanningPolicy::default();
        policy.rules.push(AutoloopRule {
            theme_id: 1,
            role_id: "drop".to_owned(),
            variant_id: "pink".to_owned(),
            enabled: true,
            selection_weight: 2,
            color_behavior: ColorBehavior::Only,
            color_rgb: vec![0xff00ff],
        });
        let result = compile(
            &policy,
            1,
            Some(0xff00ff),
            9,
            &[phrase(0)],
            &[candidate("blue", 1), candidate("pink", 2)],
            &VariationHistory::default(),
        )?;
        assert_eq!(result.choices[0].variant_id, "pink");
        assert_eq!(result.choices[0].evidence.reason, "track color only");
        Ok(())
    }

    #[test]
    fn fixed_choice_wins_over_repeat_protection() -> Result<(), LightPlanError> {
        let policy = LightPlanningPolicy::default();
        let candidates = [candidate("fixed", 4), candidate("other", 5)];
        let request = PhraseRequest {
            phrase_index: 0,
            role_id: "drop".to_owned(),
            selection: PhraseSelection::FixedVariant("fixed".to_owned()),
        };
        let result = compile(
            &policy,
            1,
            None,
            1,
            &[request],
            &candidates,
            &VariationHistory::default(),
        )?;
        assert_eq!(result.choices[0].variant_id, "fixed");
        assert_eq!(result.choices[0].evidence.reason, "track fixed variant");
        Ok(())
    }

    #[test]
    fn modifier_execution_is_fail_closed_until_both_capabilities_are_verified() {
        let modifier = OutputModifier {
            id: "dark-stage".to_owned(),
            provider_kind: "soundswitch".to_owned(),
            kind: ModifierKind::Atmosphere,
            display_name: "Dark Stage".to_owned(),
            enabled: true,
            midi_channel: 15,
            midi_note: 1,
            activation_verified: true,
            release_verified: false,
        };
        assert!(!modifier.automatic_execution_ready());
    }

    fn verified_static_modifier(scope: ModifierScope) -> (OutputModifier, ModifierRule) {
        let modifier = OutputModifier {
            id: "dark-stage".to_owned(),
            provider_kind: "soundswitch".to_owned(),
            kind: ModifierKind::Atmosphere,
            display_name: "Dark Stage".to_owned(),
            enabled: true,
            midi_channel: 12,
            midi_note: 64,
            activation_verified: true,
            release_verified: true,
        };
        let rule = ModifierRule {
            modifier_id: modifier.id.clone(),
            role_id: "drop".to_owned(),
            application_rate: 100,
            selection_weight: 2,
            cooldown_uses: 0,
            scope,
            color_behavior: ColorBehavior::Neutral,
            color_rgb: Vec::new(),
        };
        (modifier, rule)
    }

    #[test]
    fn verified_static_modifier_is_compiled_for_its_phrase_only() -> Result<(), LightPlanError> {
        let (modifier, rule) = verified_static_modifier(ModifierScope::Phrase);
        let policy = LightPlanningPolicy {
            modifiers: vec![modifier],
            modifier_rules: vec![rule],
            ..LightPlanningPolicy::default()
        };
        let result = compile(
            &policy,
            1,
            None,
            42,
            &[phrase(0), phrase(1)],
            &[candidate("a", 1)],
            &VariationHistory::default(),
        )?;
        assert_eq!(result.modifier_choices.len(), 2);
        assert_eq!(result.modifier_choices[0].modifier_id, "dark-stage");
        assert_eq!(result.modifier_choices[1].modifier_id, "dark-stage");
        Ok(())
    }

    #[test]
    fn unverified_static_modifier_is_never_compiled() -> Result<(), LightPlanError> {
        let (mut modifier, rule) = verified_static_modifier(ModifierScope::Phrase);
        modifier.release_verified = false;
        let policy = LightPlanningPolicy {
            modifiers: vec![modifier],
            modifier_rules: vec![rule],
            ..LightPlanningPolicy::default()
        };
        let result = compile(
            &policy,
            1,
            None,
            42,
            &[phrase(0)],
            &[candidate("a", 1)],
            &VariationHistory::default(),
        )?;
        assert!(result.modifier_choices.is_empty());
        Ok(())
    }

    #[test]
    fn whole_track_modifier_continues_after_first_matching_phrase() -> Result<(), LightPlanError> {
        let (modifier, rule) = verified_static_modifier(ModifierScope::Track);
        let policy = LightPlanningPolicy {
            modifiers: vec![modifier],
            modifier_rules: vec![rule],
            ..LightPlanningPolicy::default()
        };
        let phrases = [
            PhraseRequest {
                phrase_index: 0,
                role_id: "intro".to_owned(),
                selection: PhraseSelection::Automatic,
            },
            phrase(1),
            PhraseRequest {
                phrase_index: 2,
                role_id: "outro".to_owned(),
                selection: PhraseSelection::Automatic,
            },
        ];
        let candidates = [
            Candidate {
                theme_id: 1,
                role_id: "intro".to_owned(),
                variant_id: "intro".to_owned(),
                entry_id: "intro".to_owned(),
                display_name: "Intro".to_owned(),
                autoloop_number: 1,
            },
            candidate("drop", 2),
            Candidate {
                theme_id: 1,
                role_id: "outro".to_owned(),
                variant_id: "outro".to_owned(),
                entry_id: "outro".to_owned(),
                display_name: "Outro".to_owned(),
                autoloop_number: 3,
            },
        ];
        let result = compile(
            &policy,
            1,
            None,
            42,
            &phrases,
            &candidates,
            &VariationHistory::default(),
        )?;
        assert_eq!(
            result
                .modifier_choices
                .iter()
                .map(|choice| choice.phrase_index)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        Ok(())
    }

    #[test]
    fn next_track_reservation_is_released_or_committed_explicitly() -> Result<(), LightPlanError> {
        let plan = compile(
            &LightPlanningPolicy::default(),
            1,
            None,
            17,
            &[phrase(0)],
            &[candidate("reserved", 1)],
            &VariationHistory::default(),
        )?;
        let mut history = VariationHistory::default();
        history.reserve("next", &plan);
        assert!(history.theme_is_recent(1, 1));
        history.release("next");
        assert!(!history.theme_is_recent(1, 1));
        history.reserve("current", &plan);
        history.commit("current");
        assert!(history.theme_is_recent(1, 1));
        Ok(())
    }

    #[test]
    fn whole_plan_signature_ignores_seed_and_retry_avoids_recent_result()
    -> Result<(), LightPlanError> {
        let policy = LightPlanningPolicy {
            autoloop_cooldown_uses: 0,
            ..LightPlanningPolicy::default()
        };
        let candidates = [candidate("a", 1), candidate("b", 2), candidate("c", 3)];
        let phrases = [phrase(0), phrase(1)];
        let first = compile(
            &policy,
            1,
            None,
            23,
            &phrases,
            &candidates,
            &VariationHistory::default(),
        )?;
        let mut history = VariationHistory::default();
        history.reserve("current", &first);
        history.commit("current");
        let next = compile(&policy, 1, None, 23, &phrases, &candidates, &history)?;
        assert_ne!(next.signature, first.signature);
        assert_ne!(next.choices, first.choices);
        Ok(())
    }

    #[test]
    fn compilation_is_bounded_for_five_hundred_twelve_phrases() -> Result<(), LightPlanError> {
        let policy = LightPlanningPolicy::default();
        let candidates = (1..=32)
            .map(|number| candidate(&format!("v{number}"), number))
            .collect::<Vec<_>>();
        let phrases = (0..512).map(phrase).collect::<Vec<_>>();
        let started = std::time::Instant::now();
        let result = compile(
            &policy,
            1,
            None,
            101,
            &phrases,
            &candidates,
            &VariationHistory::default(),
        )?;
        assert_eq!(result.choices.len(), 512);
        let budget = if cfg!(debug_assertions) { 50 } else { 10 };
        assert!(started.elapsed() < std::time::Duration::from_millis(budget));
        Ok(())
    }
}
