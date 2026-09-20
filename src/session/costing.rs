use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::config::PricingConfig;
use crate::cost::{self, CostComputation, PricingStatus, TokenUsage};
use crate::model::SessionSpeed;

#[derive(Debug, Default)]
pub(super) struct SessionCostTracker {
    samples: Vec<Sample>,
    previous: Option<TokenUsage>,
    latest: Option<DateTime<Utc>>,
    seen: HashSet<String>,
}

#[derive(Debug)]
struct Sample {
    model: String,
    speed: SessionSpeed,
    usage: TokenUsage,
    prompt_input: Option<u64>,
    complete: bool,
}

fn usage(value: &Value) -> Option<TokenUsage> {
    Some(TokenUsage {
        input_tokens: value.get("input_tokens")?.as_u64()?,
        cached_input_tokens: value
            .get("cached_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cache_write_tokens: value
            .get("cache_write_input_tokens")
            .or_else(|| value.get("cache_write_tokens"))
            .and_then(Value::as_u64),
        output_tokens: value.get("output_tokens")?.as_u64()?,
    })
}

impl SessionCostTracker {
    pub(super) fn observe(
        &mut self,
        payload: &Value,
        timestamp: Option<DateTime<Utc>>,
        model: Option<&str>,
        speed: SessionSpeed,
    ) {
        if timestamp
            .zip(self.latest)
            .is_some_and(|(next, latest)| next < latest)
        {
            return;
        }
        let info = &payload["info"];
        let total = usage(&info["total_token_usage"]);
        let last = usage(&info["last_token_usage"]);
        let Some(current) = total.or(last) else {
            return;
        };
        let signature = format!("{timestamp:?}:{}:{}", model.unwrap_or(""), info);
        if !self.seen.insert(signature) {
            return;
        }
        let delta = match (total, self.previous) {
            (Some(total), Some(previous))
                if total.input_tokens >= previous.input_tokens
                    && total.output_tokens >= previous.output_tokens
                    && total.cached_input_tokens >= previous.cached_input_tokens
                    && total
                        .cache_write_tokens
                        .zip(previous.cache_write_tokens)
                        .is_none_or(|(a, b)| a >= b) =>
            {
                TokenUsage {
                    input_tokens: total.input_tokens - previous.input_tokens,
                    cached_input_tokens: total.cached_input_tokens - previous.cached_input_tokens,
                    cache_write_tokens: total
                        .cache_write_tokens
                        .zip(previous.cache_write_tokens)
                        .map(|(a, b)| a - b),
                    output_tokens: total.output_tokens - previous.output_tokens,
                }
            }
            _ => current,
        };
        self.previous = total;
        self.latest = timestamp.or(self.latest);
        if delta.input_tokens == 0
            && delta.output_tokens == 0
            && delta.cached_input_tokens == 0
            && delta.cache_write_tokens.unwrap_or(0) == 0
        {
            return;
        }
        let aligns = last.is_some_and(|last| {
            last.input_tokens == delta.input_tokens
                && last.output_tokens == delta.output_tokens
                && last.cached_input_tokens == delta.cached_input_tokens
        });
        let complete = total.is_some()
            && info["total_token_usage"]
                .get("cached_input_tokens")
                .is_some();
        self.samples.push(Sample {
            model: model.unwrap_or("").to_owned(),
            speed,
            usage: delta,
            prompt_input: aligns.then_some(delta.input_tokens),
            complete,
        });
    }

    pub(super) fn cache_write_tokens(&self) -> Option<u64> {
        self.samples
            .iter()
            .try_fold(0u64, |sum, sample| {
                sum.checked_add(sample.usage.cache_write_tokens?)
            })
            .filter(|_| !self.samples.is_empty())
    }

    pub(super) fn compute(&self, config: &PricingConfig) -> CostComputation {
        let mut result =
            cost::compute_cost("", TokenUsage::default(), SessionSpeed::default(), config);
        let mut partial = false;
        for sample in &self.samples {
            let item = cost::compute_cost_for_request(
                &sample.model,
                sample.usage,
                sample.speed,
                config,
                sample.prompt_input,
            );
            partial |= !sample.complete || item.status != PricingStatus::Exact;
            let Some(total) = item.known_total_cost_usd else {
                continue;
            };
            if result.known_total_cost_usd.is_none() {
                result = item;
            } else {
                result.breakdown.input_cost_usd += item.breakdown.input_cost_usd;
                result.breakdown.cached_input_cost_usd += item.breakdown.cached_input_cost_usd;
                result.breakdown.cache_write_cost_usd += item.breakdown.cache_write_cost_usd;
                result.breakdown.output_cost_usd += item.breakdown.output_cost_usd;
                result.breakdown.cached_input_savings_usd +=
                    item.breakdown.cached_input_savings_usd;
                result.total_cost_usd += total;
                result.known_total_cost_usd = result.breakdown.known_component_total();
                if result.source != item.source {
                    result.source = cost::PricingSource::Partial;
                }
                if result.resolved_model != item.resolved_model {
                    result.resolved_model = None;
                    result.pricing = None;
                }
            }
        }
        if result.known_total_cost_usd.is_none() {
            result.status = PricingStatus::Unavailable;
        } else if partial {
            result.status = PricingStatus::Partial;
        } else {
            result.status = PricingStatus::Exact;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SpeedMode, SpeedSource};
    fn event(input: u64, last: u64, write: u64) -> Value {
        serde_json::json!({"info": {
            "total_token_usage": {"input_tokens": input, "cached_input_tokens": 0, "cache_write_input_tokens": write, "output_tokens": input/100},
            "last_token_usage": {"input_tokens": last, "cached_input_tokens": 0, "cache_write_input_tokens": write, "output_tokens": last/100}
        }})
    }
    fn speed() -> SessionSpeed {
        SessionSpeed::explicit(SpeedMode::Standard, SpeedSource::ThreadSettings)
    }
    #[test]
    fn accumulated_input_does_not_become_a_long_prompt_and_duplicates_do_not_add_cost() {
        let mut tracker = SessionCostTracker::default();
        for n in 1..=5 {
            tracker.observe(
                &event(n * 100_000, 100_000, 0),
                None,
                Some("gpt-6-astra"),
                speed(),
            );
        }
        let first = tracker.compute(&PricingConfig::default());
        tracker.observe(
            &event(500_000, 100_000, 0),
            None,
            Some("gpt-6-astra"),
            speed(),
        );
        assert_eq!(first.status, PricingStatus::Exact);
        assert_eq!(
            first.known_total_cost_usd,
            tracker
                .compute(&PricingConfig::default())
                .known_total_cost_usd
        );
        assert!((first.total_cost_usd - 5.25).abs() < 1e-9);
    }
    #[test]
    fn cache_write_and_mixed_model_subtotals_survive_unknown_models() {
        let mut tracker = SessionCostTracker::default();
        tracker.observe(
            &event(100_000, 100_000, 10_000),
            None,
            Some("gpt-6-astra"),
            speed(),
        );
        let first = tracker.compute(&PricingConfig::default());
        assert!(first.breakdown.cache_write_cost_usd > 0.0);
        assert_eq!(tracker.cache_write_tokens(), Some(10_000));
        tracker.observe(
            &event(200_000, 100_000, 10_000),
            None,
            Some("unknown-model"),
            speed(),
        );
        let next = tracker.compute(&PricingConfig::default());
        assert_eq!(next.status, PricingStatus::Partial);
        assert_eq!(first.known_total_cost_usd, next.known_total_cost_usd);
    }
    #[test]
    fn missing_request_history_is_partial_but_observed_long_requests_use_their_tier() {
        let mut tracker = SessionCostTracker::default();
        tracker.observe(
            &event(900_000, 100_000, 0),
            None,
            Some("gpt-6-astra"),
            speed(),
        );
        assert_eq!(
            tracker.compute(&PricingConfig::default()).status,
            PricingStatus::Partial
        );
        let mut tracker = SessionCostTracker::default();
        tracker.observe(
            &event(300_000, 300_000, 0),
            None,
            Some("gpt-6-astra"),
            speed(),
        );
        let long = tracker.compute(&PricingConfig::default());
        assert_eq!(long.status, PricingStatus::Exact);
        assert!((long.total_cost_usd - 6.225).abs() < 1e-9);
    }

    #[test]
    fn model_and_speed_changes_keep_each_sample_at_its_observed_rate() {
        let mut tracker = SessionCostTracker::default();
        tracker.observe(
            &event(100_000, 100_000, 0),
            None,
            Some("gpt-6-astra"),
            speed(),
        );
        tracker.observe(
            &event(200_000, 100_000, 0),
            None,
            Some("gpt-6-astra"),
            SessionSpeed::explicit(SpeedMode::Fast, SpeedSource::ThreadSettings),
        );
        let subtotal = tracker.compute(&PricingConfig::default());
        assert!((subtotal.total_cost_usd - 3.15).abs() < 1e-9);
        tracker.observe(&event(300_000, 100_000, 0), None, Some("gpt-5.4"), speed());
        let mixed = tracker.compute(&PricingConfig::default());
        let third = cost::compute_cost_for_request(
            "gpt-5.4",
            usage(&event(100_000, 100_000, 0)["info"]["total_token_usage"]).unwrap(),
            speed(),
            &PricingConfig::default(),
            Some(100_000),
        );
        assert!(
            (mixed.total_cost_usd - subtotal.total_cost_usd - third.total_cost_usd).abs() < 1e-9
        );
        assert!(mixed.resolved_model.is_none());
    }
}
