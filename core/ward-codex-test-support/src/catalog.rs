// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::state::FakeState;

const SECOND_MODEL_PAGE_CURSOR: &str = "models-page-2";

#[derive(Clone, Copy, Eq, PartialEq)]
enum FakeModelPage {
    First,
    Second,
}

struct FakeReasoningEffort {
    value: &'static str,
    description: &'static str,
}

pub(super) struct FakeModelDefinition {
    id: &'static str,
    pub(super) model: &'static str,
    display_name: &'static str,
    description: &'static str,
    hidden: bool,
    is_default: bool,
    pub(super) default_reasoning_effort: &'static str,
    supported_reasoning_efforts: &'static [FakeReasoningEffort],
    page: FakeModelPage,
}

impl FakeModelDefinition {
    pub(super) fn supports_reasoning_effort(&self, effort: &str) -> bool {
        self.supported_reasoning_efforts
            .iter()
            .any(|option| option.value == effort)
    }

    fn to_json(&self) -> Value {
        let supported_reasoning_efforts = self
            .supported_reasoning_efforts
            .iter()
            .map(|option| {
                json!({
                    "reasoningEffort": option.value,
                    "description": option.description,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "id": self.id,
            "model": self.model,
            "displayName": self.display_name,
            "description": self.description,
            "hidden": self.hidden,
            "isDefault": self.is_default,
            "defaultReasoningEffort": self.default_reasoning_effort,
            "supportedReasoningEfforts": supported_reasoning_efforts,
        })
    }
}

const BALANCED_REASONING_EFFORTS: &[FakeReasoningEffort] = &[
    FakeReasoningEffort {
        value: "low",
        description: "Faster responses",
    },
    FakeReasoningEffort {
        value: "medium",
        description: "Balanced reasoning",
    },
    FakeReasoningEffort {
        value: "high",
        description: "Deeper reasoning",
    },
];
const FAST_REASONING_EFFORTS: &[FakeReasoningEffort] = &[
    FakeReasoningEffort {
        value: "low",
        description: "Faster responses",
    },
    FakeReasoningEffort {
        value: "medium",
        description: "Balanced reasoning",
    },
];
const INTERNAL_REASONING_EFFORTS: &[FakeReasoningEffort] = &[FakeReasoningEffort {
    value: "medium",
    description: "Balanced reasoning",
}];
const FAKE_MODELS: &[FakeModelDefinition] = &[
    FakeModelDefinition {
        id: "balanced",
        model: "gpt-balanced",
        display_name: "Balanced",
        description: "Balances capability and speed.",
        hidden: false,
        is_default: true,
        default_reasoning_effort: "medium",
        supported_reasoning_efforts: BALANCED_REASONING_EFFORTS,
        page: FakeModelPage::First,
    },
    FakeModelDefinition {
        id: "internal",
        model: "gpt-internal",
        display_name: "Internal",
        description: "Hidden test model.",
        hidden: true,
        is_default: false,
        default_reasoning_effort: "medium",
        supported_reasoning_efforts: INTERNAL_REASONING_EFFORTS,
        page: FakeModelPage::First,
    },
    FakeModelDefinition {
        id: "fast",
        model: "gpt-fast",
        display_name: "Fast",
        description: "Optimized for quick iteration.",
        hidden: false,
        is_default: false,
        default_reasoning_effort: "low",
        supported_reasoning_efforts: FAST_REASONING_EFFORTS,
        page: FakeModelPage::Second,
    },
];

pub(super) fn fake_model(model: &str) -> Option<&'static FakeModelDefinition> {
    FAKE_MODELS
        .iter()
        .find(|definition| definition.model == model)
}

pub(super) fn default_fake_model() -> &'static FakeModelDefinition {
    FAKE_MODELS
        .iter()
        .find(|model| model.is_default)
        .expect("the fake model catalog must declare a default")
}

pub(super) fn model_list_response(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
) -> Value {
    let mut state = state.lock().unwrap();
    if state.options.model_list_failures > 0 {
        state.options.model_list_failures -= 1;
        return json!({
            "id": id,
            "error": {
                "code": -32603,
                "message": "the model catalog is temporarily unavailable"
            }
        });
    }
    drop(state);

    let (page, next_cursor) = match params.get("cursor").and_then(Value::as_str) {
        None => (FakeModelPage::First, Some(SECOND_MODEL_PAGE_CURSOR)),
        Some(SECOND_MODEL_PAGE_CURSOR) => (FakeModelPage::Second, None),
        Some(cursor) => {
            return json!({
                "id": id,
                "error": {
                    "code": -32600,
                    "message": format!("unknown model-list cursor: {cursor}")
                }
            });
        }
    };
    let include_hidden = params
        .get("includeHidden")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let data = FAKE_MODELS
        .iter()
        .filter(|model| model.page == page && (include_hidden || !model.hidden))
        .map(FakeModelDefinition::to_json)
        .collect::<Vec<_>>();
    json!({
        "id": id,
        "result": {
            "data": data,
            "nextCursor": next_cursor,
        }
    })
}
