// src/effects/effects_manager.rs
//
// this pattern IS NO LONGER USED.

use crate::effects::*;
use crate::views::DrawStyle;
use std::collections::HashMap;

#[derive(Default)]
pub struct EffectsManager {
    effects: HashMap<String, EffectInstance>,
}

impl EffectsManager {
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
        }
    }

    // Add a new effect
    pub fn add(&mut self, name: String, effect: EffectType) {
        self.effects.insert(
            name,
            EffectInstance {
                effect,
                is_active: true,
            },
        );
    }

    // Remove an effect
    pub fn remove(&mut self, name: &str) {
        self.effects.remove(name);
    }

    // toggle an effect
    pub fn activate(&mut self, name: &str) {
        if let Some(instance) = self.effects.get_mut(name) {
            instance.is_active = true;
        }
    }

    pub fn deactivate(&mut self, name: &str) {
        if let Some(instance) = self.effects.get_mut(name) {
            instance.is_active = false;
        }
    }

    // Apply all active effects
    pub fn apply_backbone_effects(&self, base_style: DrawStyle, time: f32) -> DrawStyle {
        if self.effects.is_empty() {
            return base_style;
        }

        let mut current_style = base_style;

        for instance in self.effects.values() {
            if !instance.is_active {
                continue;
            }

            match &instance.effect {
                EffectType::Backbone(effect) => {
                    current_style = effect.update(&current_style, time);
                }
            }
        }

        current_style
    }

    // Clean up finished effects
    pub fn cleanup(&mut self) {
        self.effects.retain(|_, instance| match &instance.effect {
            EffectType::Backbone(effect) => !effect.is_finished(),
        });
    }

    // Utility functions
    pub fn is_active(&self, name: &str) -> bool {
        self.effects
            .get(name)
            .map(|instance| instance.is_active)
            .unwrap_or(false)
    }

    pub fn has_effect(&self, name: &str) -> bool {
        self.effects.contains_key(name)
    }
}
