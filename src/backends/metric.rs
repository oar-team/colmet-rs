
#[derive(Debug, Clone)]
pub struct MetricValues {
    pub job_id: i32,
    pub backend_name: String,
    pub metric_names: Vec<String>,
    pub metric_values: Option<Vec<i64>>,
}

#[derive(Debug, Clone)]
pub struct Metric {
    pub job_id: i32,
    pub metric_name: String,
    pub backend_name: String,
    pub sampling_period: f32,
    pub time_remaining_before_next_measure: i64,
}
