//! Canonical sales pipeline aggregate. No HTML/SQL-specific rules live here.
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineStage {
    pub stage_id: String,
    pub name: String,
    pub ordinal: u16,
    pub probability_basis_points: u16,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesPipeline {
    pub pipeline_id: String,
    pub name: String,
    pub version: i64,
    pub archived: bool,
    pub stages: Vec<PipelineStage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineStageInput {
    pub stage_id: String,
    pub name: String,
    pub probability_basis_points: u16,
    pub archived: bool,
}

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, "Invalid sales pipeline.")
}

fn conflict(code: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, "Sales pipeline version conflict.")
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 180
        && value == value.trim()
        && !value.contains('/')
        && !value.chars().any(char::is_control)
}

fn title(value: &str) -> bool {
    !value.trim().is_empty() && value == value.trim() && value.len() <= 240
}

impl SalesPipeline {
    pub fn create(
        pipeline_id: String,
        name: String,
        stages: Vec<PipelineStageInput>,
    ) -> Result<Self, SdkError> {
        if !identifier(&pipeline_id) {
            return Err(invalid("SALES_PIPELINE_ID_INVALID"));
        }
        let stages = Self::validate_stages(stages)?;
        if !title(&name) {
            return Err(invalid("SALES_PIPELINE_NAME_INVALID"));
        }
        Ok(Self {
            pipeline_id,
            name,
            version: 1,
            archived: false,
            stages,
        })
    }

    pub fn reconfigure(
        &self,
        expected_version: i64,
        name: String,
        archived: bool,
        stages: Vec<PipelineStageInput>,
    ) -> Result<Self, SdkError> {
        if expected_version != self.version {
            return Err(conflict("SALES_PIPELINE_VERSION_CONFLICT"));
        }
        if !title(&name) {
            return Err(invalid("SALES_PIPELINE_NAME_INVALID"));
        }
        if archived && !self.archived {
            // Storage-level reference validation is mandatory before archiving.
            // This aggregate makes no unsupported assumptions about linked Deals.
        }
        let stages = Self::validate_stages(stages)?;
        Ok(Self {
            pipeline_id: self.pipeline_id.clone(),
            name,
            archived,
            stages,
            version: self
                .version
                .checked_add(1)
                .ok_or_else(|| conflict("SALES_PIPELINE_VERSION_OVERFLOW"))?,
        })
    }

    fn validate_stages(input: Vec<PipelineStageInput>) -> Result<Vec<PipelineStage>, SdkError> {
        if input.is_empty() || input.len() > 24 {
            return Err(invalid("SALES_PIPELINE_STAGES_COUNT_INVALID"));
        }
        let mut ids = BTreeSet::new();
        input
            .into_iter()
            .enumerate()
            .map(|(index, stage)| {
                if !identifier(&stage.stage_id) || !title(&stage.name) {
                    return Err(invalid("SALES_PIPELINE_STAGE_INVALID"));
                }
                if !ids.insert(stage.stage_id.clone()) {
                    return Err(invalid("SALES_PIPELINE_STAGE_DUPLICATE"));
                }
                if stage.probability_basis_points > 10_000 {
                    return Err(invalid("SALES_PIPELINE_PROBABILITY_INVALID"));
                }
                Ok(PipelineStage {
                    stage_id: stage.stage_id,
                    name: stage.name,
                    ordinal: (index + 1) as u16,
                    probability_basis_points: stage.probability_basis_points,
                    archived: stage.archived,
                })
            })
            .collect()
    }

    pub fn stage(&self, id: &str) -> Option<&PipelineStage> {
        self.stages.iter().find(|stage| stage.stage_id == id)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, SdkError> {
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| invalid("SALES_PIPELINE_STATE_INVALID"))?;
        if value.version < 1 || !identifier(&value.pipeline_id) || !title(&value.name) {
            return Err(invalid("SALES_PIPELINE_STATE_INVALID"));
        }
        let checked = Self::validate_stages(
            value
                .stages
                .iter()
                .map(|s| PipelineStageInput {
                    stage_id: s.stage_id.clone(),
                    name: s.name.clone(),
                    probability_basis_points: s.probability_basis_points,
                    archived: s.archived,
                })
                .collect(),
        )?;
        if checked != value.stages {
            return Err(invalid("SALES_PIPELINE_STAGE_ORDER_INVALID"));
        }
        Ok(value)
    }

    pub fn encode(&self) -> Result<String, SdkError> {
        serde_json::to_string(self).map_err(|_| invalid("SALES_PIPELINE_ENCODE_FAILED"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> Vec<PipelineStageInput> {
        vec![
            PipelineStageInput {
                stage_id: "lead".into(),
                name: "Новый контакт".into(),
                probability_basis_points: 1000,
                archived: false,
            },
            PipelineStageInput {
                stage_id: "qualified".into(),
                name: "Квалификация".into(),
                probability_basis_points: 4500,
                archived: false,
            },
        ]
    }

    #[test]
    fn stages_use_owner_ordinals_and_stable_ids() {
        let mut pipeline = SalesPipeline::create("p-1".into(), "Основная".into(), draft()).unwrap();
        assert_eq!(pipeline.stage("qualified").unwrap().ordinal, 2);
        assert_eq!(SalesPipeline::decode(pipeline.encode().unwrap().as_bytes()).unwrap(), pipeline);
        let original = pipeline.clone();
        let mut reordered = draft();
        reordered.reverse();
        pipeline = pipeline.reconfigure(1, "Обновлённая".into(), false, reordered).unwrap();
        assert_eq!(original.stage("lead").unwrap().ordinal, 1);
        assert_eq!(pipeline.stage("lead").unwrap().ordinal, 2);
        assert_eq!(pipeline.version, 2);
        assert_eq!(
            pipeline.reconfigure(1, "Нет".into(), false, draft()).unwrap_err().code,
            "SALES_PIPELINE_VERSION_CONFLICT"
        );
    }

    #[test]
    fn validation_is_fail_closed() {
        let mut duplicated = draft();
        duplicated[1].stage_id = "lead".into();
        assert_eq!(
            SalesPipeline::create("p-1".into(), "Основная".into(), duplicated).unwrap_err().code,
            "SALES_PIPELINE_STAGE_DUPLICATE"
        );
        let mut bad = draft();
        bad[0].probability_basis_points = 10_001;
        assert_eq!(
            SalesPipeline::create("p-1".into(), "Основная".into(), bad).unwrap_err().code,
            "SALES_PIPELINE_PROBABILITY_INVALID"
        );
        let mut pipeline = SalesPipeline::create("p-1".into(), "Основная".into(), draft()).unwrap();
        pipeline.stages[0].ordinal = 99;
        assert_eq!(
            SalesPipeline::decode(pipeline.encode().unwrap().as_bytes()).unwrap_err().code,
            "SALES_PIPELINE_STAGE_ORDER_INVALID"
        );
    }
}
