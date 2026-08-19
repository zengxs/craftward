// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::{ModelInfo, ReasoningEffort, ReasoningEffortOption};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelListParams<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<&'a str>,
    include_hidden: bool,
}

impl<'a> ModelListParams<'a> {
    pub(crate) const fn visible(cursor: Option<&'a str>) -> Self {
        Self {
            cursor,
            include_hidden: false,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelListResponse {
    data: Vec<WireModel>,
    #[serde(default)]
    next_cursor: Option<String>,
}

impl ModelListResponse {
    pub(crate) fn into_parts(self) -> (Vec<ModelInfo>, Option<String>) {
        let models = self
            .data
            .into_iter()
            .filter_map(WireModel::into_visible_model)
            .collect();
        (models, self.next_cursor)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireModel {
    id: String,
    model: String,
    display_name: String,
    description: String,
    hidden: bool,
    is_default: bool,
    default_reasoning_effort: WireReasoningEffort,
    supported_reasoning_efforts: Vec<WireReasoningEffortOption>,
}

impl WireModel {
    fn into_visible_model(self) -> Option<ModelInfo> {
        (!self.hidden).then(|| ModelInfo {
            id: self.id,
            model: self.model,
            display_name: self.display_name,
            description: self.description,
            is_default: self.is_default,
            default_reasoning_effort: self.default_reasoning_effort.into_model(),
            supported_reasoning_efforts: self
                .supported_reasoning_efforts
                .into_iter()
                .map(WireReasoningEffortOption::into_model)
                .collect(),
        })
    }
}

#[derive(Deserialize)]
#[serde(try_from = "String")]
struct WireReasoningEffort(String);

impl TryFrom<String> for WireReasoningEffort {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err("a reasoning effort must not be empty")
        } else {
            Ok(Self(value))
        }
    }
}

impl WireReasoningEffort {
    fn into_model(self) -> ReasoningEffort {
        ReasoningEffort::new(self.0).expect("wire reasoning efforts were validated while decoding")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireReasoningEffortOption {
    reasoning_effort: WireReasoningEffort,
    description: String,
}

impl WireReasoningEffortOption {
    fn into_model(self) -> ReasoningEffortOption {
        ReasoningEffortOption {
            effort: self.reasoning_effort.into_model(),
            description: self.description,
        }
    }
}
