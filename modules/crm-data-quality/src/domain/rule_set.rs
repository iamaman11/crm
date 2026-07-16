#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRuleSetVersion {
    version_id: PartyRuleSetVersionId,
    rules: Vec<PartyQualityRule>,
}

#[derive(Serialize)]
struct CanonicalRuleSet<'a> {
    target_resource_type: &'static str,
    evaluator_semantic_version: &'static str,
    rules: &'a [PartyQualityRule],
}

impl PartyRuleSetVersion {
    pub fn publish(mut rules: Vec<PartyQualityRule>) -> Result<Self, SdkError> {
        if rules.is_empty() || rules.len() > MAX_RULES_PER_RULE_SET {
            return Err(invalid(
                "DATA_QUALITY_RULE_SET_SIZE_INVALID",
                "rule_set.rules",
                format!("rule count must be in the inclusive range 1..={MAX_RULES_PER_RULE_SET}"),
            ));
        }

        rules.sort_by(|left, right| left.rule_key.cmp(&right.rule_key));
        if rules
            .windows(2)
            .any(|pair| pair[0].rule_key == pair[1].rule_key)
        {
            return Err(invalid(
                "DATA_QUALITY_RULE_KEY_DUPLICATE",
                "rule_set.rules",
                "rule keys must be unique within one published rule-set version",
            ));
        }

        let canonical = CanonicalRuleSet {
            target_resource_type: TARGET_RESOURCE_TYPE,
            evaluator_semantic_version: EVALUATOR_SEMANTIC_VERSION,
            rules: &rules,
        };
        let version_id =
            PartyRuleSetVersionId::from_digest(&canonical_digest(RULE_SET_ID_DOMAIN, &canonical));

        Ok(Self { version_id, rules })
    }

    pub fn version_id(&self) -> &PartyRuleSetVersionId {
        &self.version_id
    }

    pub fn rules(&self) -> &[PartyQualityRule] {
        &self.rules
    }

    pub fn rule(&self, rule_key: &RuleKey) -> Option<&PartyQualityRule> {
        self.rules
            .binary_search_by(|rule| rule.rule_key.cmp(rule_key))
            .ok()
            .map(|index| &self.rules[index])
    }

    pub fn evaluate(&self, input: &PartyQualityInput) -> Vec<PartyRuleEvaluation> {
        self.rules
            .iter()
            .map(|rule| {
                let result = rule.evaluator.evaluate(input);
                PartyRuleEvaluation {
                    rule_set_version_id: self.version_id.clone(),
                    rule_key: rule.rule_key.clone(),
                    passed: result.passed,
                    reason_code: result.reason_code.to_owned(),
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRuleEvaluation {
    rule_set_version_id: PartyRuleSetVersionId,
    rule_key: RuleKey,
    passed: bool,
    reason_code: String,
}

impl PartyRuleEvaluation {
    pub fn rule_set_version_id(&self) -> &PartyRuleSetVersionId {
        &self.rule_set_version_id
    }

    pub fn rule_key(&self) -> &RuleKey {
        &self.rule_key
    }

    pub const fn passed(&self) -> bool {
        self.passed
    }

    pub fn reason_code(&self) -> &str {
        self.reason_code.as_str()
    }
}
