use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{interface::types::location::NewLocation, model::models};

#[derive(Deserialize, Serialize, Debug)]
pub struct NewProducer {
    pub location: NewLocation,
    pub group_id: Uuid,
    pub name: String,
    pub description: String,
    pub forwarder_id: Option<Uuid>,
}

// TODO: Don't wrap it, complete new type
#[derive(Serialize, Deserialize, Debug)]
pub struct Producer(models::Producer);
