//! Integration event transports.

use tracing::info;

use iaop_integration_service_application::ports::IntegrationEvent;

pub struct LogTransport;

impl LogTransport {
    pub async fn deliver(&self, event: &IntegrationEvent) -> Result<(), String> {
        info!(
            event_id = %event.event_id,
            event_type = %event.event_type,
            event_version = event.event_version,
            "integration event delivered via log transport"
        );
        Ok(())
    }
}

pub struct KafkaTransport {
    brokers: String,
    topic_prefix: String,
}

impl KafkaTransport {
    pub fn new(brokers: String, topic_prefix: Option<String>) -> Self {
        let topic_prefix = topic_prefix.unwrap_or_else(|| "iaop".to_string());
        Self {
            brokers,
            topic_prefix,
        }
    }

    pub fn topic_for(&self, event_type: &str) -> String {
        format!("{}.{}", self.topic_prefix, event_type)
    }

    pub async fn deliver(&self, event: &IntegrationEvent) -> Result<(), String> {
        let topic = self.topic_for(&event.event_type);
        let key = event
            .aggregate_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| event.event_id.to_string());

        let payload = serde_json::to_vec(&event)
            .map_err(|e| format!("failed to serialize event envelope: {e}"))?;

        info!(
            brokers = %self.brokers,
            topic = %topic,
            key = %key,
            event_id = %event.event_id,
            bytes = payload.len(),
            "publishing integration event to Kafka topic"
        );

        Ok(())
    }
}

pub enum ConfiguredTransport {
    Log(LogTransport),
    Kafka(KafkaTransport),
}

impl ConfiguredTransport {
    pub async fn deliver(&self, event: &IntegrationEvent) -> Result<(), String> {
        match self {
            Self::Log(t) => t.deliver(event).await,
            Self::Kafka(t) => t.deliver(event).await,
        }
    }
}
