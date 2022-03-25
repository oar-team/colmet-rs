use serde::ser::{Serialize, Serializer, SerializeStruct};

#[derive(Debug, Clone)]
pub struct MetricValues {
    pub job_id: i32,
    pub backend_name: String,
    pub metric_names: Vec<String>,
    pub metric_values: Vec<i64>,
}

#[derive(Debug, Clone)]
pub struct Metric {
    pub job_id: i32,
    pub metric_name: String,
    pub backend_name: String,
    pub sampling_period: f32,
    pub time_remaining_before_next_measure: i64,
}

impl Serialize for MetricValues {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where 
        S:Serializer,
    {
        let mut state = serializer.serialize_struct("MetricValues", 4)?;
        state.serialize_field("job_id", &self.job_id).expect("error when serializing job_id in MetricValues");
        state.serialize_field("backend_name", &self.backend_name).expect("error when serializing backend_name in MetricValues");
        state.serialize_field("metric_names", &self.metric_names).expect("error when serializing metric_name in MetricValues");
        state.serialize_field("metric_values", &self.metric_values).expect("error when serializing metric_values in MetricValues");
        state.end()
    }
}
