use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{authorize, run_query, ApiError, AppState};

const DATABASE: &str = "closed_loop_finance";

#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    pub limit: Option<i64>,
    pub supplier_id: Option<String>,
    pub category: Option<String>,
    pub channel: Option<String>,
    pub region: Option<String>,
    pub product_id: Option<String>,
    pub period: Option<String>,
}

impl AnalyticsQuery {
    fn limit(&self) -> i64 {
        self.limit.unwrap_or(100).clamp(1, 500)
    }

    fn validate(&self) -> Result<(), ApiError> {
        for (name, value) in [
            ("supplier_id", self.supplier_id.as_deref()),
            ("category", self.category.as_deref()),
            ("channel", self.channel.as_deref()),
            ("region", self.region.as_deref()),
            ("product_id", self.product_id.as_deref()),
            ("period", self.period.as_deref()),
        ] {
            if let Some(value) = value {
                if value.is_empty() || value.len() > 200 || value.contains('\0') {
                    return Err(ApiError::bad_request(format!("Invalid {name} filter")));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct ProcurementSpend {
    pub supplier_id: String,
    pub landed_cost: f64,
    pub contract_variance: f64,
    pub quantity: f64,
}

#[derive(Debug, Serialize)]
pub struct MarginBridge {
    pub channel: String,
    pub region: String,
    pub product_id: String,
    pub revenue: f64,
    pub cogs: f64,
    pub gross_profit: f64,
    pub gross_margin: f64,
}

#[derive(Debug, Serialize)]
pub struct WorkingCapital {
    pub period: String,
    pub dso: f64,
    pub dio: f64,
    pub dpo: f64,
    pub ccc: f64,
}

#[derive(Debug, Serialize)]
pub struct ValuePool {
    pub category: String,
    pub addressable_spend: f64,
    pub savings_rate: f64,
    pub probability: f64,
    pub value_pool: f64,
}

pub async fn procurement_spend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    query.validate()?;
    let limit = query.limit();
    let rows = run_query(
        &state,
        DATABASE,
        "SELECT supplier_id, landed_cost, contract_variance, quantity \
         FROM supplier_spend \
         WHERE ($1::STRING IS NULL OR supplier_id = $1) \
         ORDER BY landed_cost DESC LIMIT $2",
        &[&query.supplier_id, &limit],
    )
    .await?;
    let data = rows
        .iter()
        .map(|row| ProcurementSpend {
            supplier_id: required_string(row, "supplier_id"),
            landed_cost: number(row, "landed_cost"),
            contract_variance: number(row, "contract_variance"),
            quantity: number(row, "quantity"),
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({ "metric": "procurement_spend", "rows": data })))
}

pub async fn margin_bridge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    query.validate()?;
    let limit = query.limit();
    let rows = run_query(
        &state,
        DATABASE,
        "SELECT channel, region, product_id, revenue, cogs, gross_profit, gross_margin \
         FROM margin_bridge \
         WHERE ($1::STRING IS NULL OR channel = $1) \
           AND ($2::STRING IS NULL OR region = $2) \
           AND ($3::STRING IS NULL OR product_id = $3) \
         ORDER BY gross_profit DESC LIMIT $4",
        &[&query.channel, &query.region, &query.product_id, &limit],
    )
    .await?;
    let data = rows
        .iter()
        .map(|row| MarginBridge {
            channel: required_string(row, "channel"),
            region: required_string(row, "region"),
            product_id: required_string(row, "product_id"),
            revenue: number(row, "revenue"),
            cogs: number(row, "cogs"),
            gross_profit: number(row, "gross_profit"),
            gross_margin: number(row, "gross_margin"),
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({ "metric": "margin_bridge", "rows": data })))
}

pub async fn working_capital(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    query.validate()?;
    let limit = query.limit();
    let rows = run_query(
        &state,
        DATABASE,
        "SELECT period, dso, dio, dpo, ccc \
         FROM working_capital_metrics \
         WHERE ($1::STRING IS NULL OR period = $1) \
         ORDER BY period DESC LIMIT $2",
        &[&query.period, &limit],
    )
    .await?;
    let data = rows
        .iter()
        .map(|row| WorkingCapital {
            period: required_string(row, "period"),
            dso: number(row, "dso"),
            dio: number(row, "dio"),
            dpo: number(row, "dpo"),
            ccc: number(row, "ccc"),
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({ "metric": "working_capital", "rows": data })))
}

pub async fn value_pools(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    query.validate()?;
    let limit = query.limit();
    let rows = run_query(
        &state,
        DATABASE,
        "SELECT category, addressable_spend, savings_rate, probability, value_pool \
         FROM value_pools \
         WHERE ($1::STRING IS NULL OR category = $1) \
         ORDER BY value_pool DESC LIMIT $2",
        &[&query.category, &limit],
    )
    .await?;
    let data = rows
        .iter()
        .map(|row| ValuePool {
            category: required_string(row, "category"),
            addressable_spend: number(row, "addressable_spend"),
            savings_rate: number(row, "savings_rate"),
            probability: number(row, "probability"),
            value_pool: number(row, "value_pool"),
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({ "metric": "value_pools", "rows": data })))
}

fn required_string(row: &Value, field: &str) -> String {
    row.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn number(row: &Value, field: &str) -> f64 {
    row.get(field)
        .and_then(crate::json_value_to_f64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytics_filters_are_bounded_and_limit_is_clamped() {
        let query = AnalyticsQuery {
            limit: Some(9_999),
            supplier_id: Some("supplier-a".into()),
            category: None,
            channel: None,
            region: None,
            product_id: None,
            period: None,
        };
        assert_eq!(query.limit(), 500);
        assert!(query.validate().is_ok());
    }

    #[test]
    fn analytics_filters_reject_embedded_nul() {
        let query = AnalyticsQuery {
            limit: None,
            supplier_id: Some("supplier\0".into()),
            category: None,
            channel: None,
            region: None,
            product_id: None,
            period: None,
        };
        assert!(query.validate().is_err());
    }

    #[test]
    fn response_mapping_keeps_numeric_strings_typed() {
        let row = json!({"landed_cost": "12.50", "supplier_id": "s1"});
        let item = ProcurementSpend {
            supplier_id: required_string(&row, "supplier_id"),
            landed_cost: number(&row, "landed_cost"),
            contract_variance: 0.0,
            quantity: 1.0,
        };
        assert_eq!(item.landed_cost, 12.5);
    }
}
